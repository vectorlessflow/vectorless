"""Orchestrator agent — coordinates multi-document retrieval.

The Orchestrator follows a 3-phase process:
1. Analyze: LLM selects documents + tasks (using DocCards and keyword search)
2. Supervisor loop: dispatch Workers → evaluate → replan if insufficient
3. Return merged evidence (answer synthesis handled separately)

Mirrors vectorless-core/vectorless-agent/src/orchestrator/.
"""

from __future__ import annotations

import asyncio
import logging
import re
from dataclasses import dataclass, field
from typing import Any

from vectorless._types import TraceStep, WorkerEvidence, WorkerResult
from vectorless.agent.evaluate import EvalResult, evaluate
from vectorless.agent.worker import Worker
from vectorless.llm_client import LLMClient
from vectorless.prompts.agent import (
    DispatchEntry,
    OrchestratorAnalysisParams,
    orchestrator_analysis,
    orchestrator_replan_prompt,
    parse_dispatch_plan,
    parse_replan_response,
)

logger = logging.getLogger(__name__)

MAX_SUPERVISOR_ITERATIONS = 3


# ---------------------------------------------------------------------------
# DocCard — lightweight document metadata for analysis
# ---------------------------------------------------------------------------

@dataclass
class DocCard:
    """Summary of an ingested document, used for orchestrator analysis."""
    doc_id: str
    name: str
    summary: str
    section_count: int
    concepts: list[str]


# ---------------------------------------------------------------------------
# Orchestrator state
# ---------------------------------------------------------------------------

@dataclass
class _OrchestratorState:
    """Mutable state for a single Orchestrator run."""
    dispatched: list[int] = field(default_factory=list)
    worker_results: list[tuple[int, WorkerResult]] = field(default_factory=list)
    all_evidence: list[WorkerEvidence] = field(default_factory=list)
    all_traces: list[TraceStep] = field(default_factory=list)
    analyze_done: bool = False
    total_llm_calls: int = 0

    def record_dispatch(self, doc_idx: int) -> None:
        if doc_idx not in self.dispatched:
            self.dispatched.append(doc_idx)

    def collect_result(self, doc_idx: int, result: WorkerResult) -> None:
        self.worker_results.append((doc_idx, result))
        self.all_evidence.extend(result.evidence)
        self.all_traces.extend(result.trace)
        self.record_dispatch(doc_idx)


# ---------------------------------------------------------------------------
# Orchestrator output
# ---------------------------------------------------------------------------

@dataclass
class OrchestratorResult:
    """Final result of the Orchestrator run."""
    evidence: list[WorkerEvidence]
    trace: list[TraceStep]
    llm_calls: int
    rounds_used: int
    nodes_visited: int
    confidence: float


# ---------------------------------------------------------------------------
# Orchestrator
# ---------------------------------------------------------------------------

class Orchestrator:
    """Coordinates multi-document retrieval with Workers.

    Usage::

        orch = Orchestrator(
            query="Compare revenue across years",
            doc_cards=[card1, card2],
            doc_loader=load_fn,
            llm_client=llm,
        )
        result = await orch.run()
    """

    def __init__(
        self,
        query: str,
        doc_cards: list[DocCard],
        doc_loader: Any,  # callable: (doc_id: str) -> PyDocument
        llm_client: LLMClient,
        *,
        max_rounds: int = 15,
        max_llm_calls: int = 0,
        skip_analysis: bool = False,
        intent_context: str = "",
        event_callback: Any = None,  # async callable: (dict) -> None
    ) -> None:
        self._query = query
        self._doc_cards = doc_cards
        self._doc_loader = doc_loader
        self._llm = llm_client
        self._max_rounds = max_rounds
        self._max_llm_calls = max_llm_calls
        self._skip_analysis = skip_analysis
        self._intent_context = intent_context
        self._emit = event_callback or (lambda _: asyncio.ensure_future(asyncio.sleep(0)))

    async def run(self) -> OrchestratorResult:
        """Execute the Orchestrator: analyze → dispatch → evaluate → replan."""
        query = self._query
        cards = self._doc_cards
        llm = self._llm
        state = _OrchestratorState()

        logger.info(
            "Orchestrator starting (docs=%d, skip_analysis=%s)",
            len(cards), self._skip_analysis,
        )

        # --- Phase 1: Analyze ---
        initial_dispatches = await self._analyze(
            query, cards, llm, state, self._skip_analysis, self._intent_context,
        )

        if initial_dispatches is None:
            # Already answered by cross-doc search
            return OrchestratorResult(
                evidence=[], trace=[], llm_calls=state.total_llm_calls,
                rounds_used=0, nodes_visited=0, confidence=0.0,
            )

        # --- Phase 2: Supervisor loop ---
        outcome = await self._supervisor_loop(
            query, initial_dispatches, cards, llm, state,
        )

        # --- Finalize ---
        confidence = _compute_confidence(
            eval_sufficient=outcome.eval_sufficient,
            replan_rounds=outcome.iteration,
            no_evidence=not state.all_evidence,
        )

        total_rounds = sum(r.rounds_used for _, r in state.worker_results)
        total_visited = sum(r.nodes_visited for _, r in state.worker_results)

        return OrchestratorResult(
            evidence=state.all_evidence,
            trace=state.all_traces,
            llm_calls=state.total_llm_calls,
            rounds_used=total_rounds,
            nodes_visited=total_visited,
            confidence=confidence,
        )

    # -----------------------------------------------------------------------
    # Phase 1: Analyze
    # -----------------------------------------------------------------------

    async def _analyze(
        self,
        query: str,
        cards: list[DocCard],
        llm: LLMClient,
        state: _OrchestratorState,
        skip_analysis: bool,
        intent_context: str,
    ) -> list[DispatchEntry] | None:
        """Analyze documents and produce a dispatch plan.

        Returns None if already answered, or list of DispatchEntry.
        """
        if skip_analysis:
            return [
                DispatchEntry(
                    doc_idx=i,
                    reason="User-specified document",
                    task=query,
                )
                for i in range(len(cards))
            ]

        # Build doc cards text
        doc_cards_text = self._format_doc_cards(cards)

        # Cross-document keyword search
        keywords = _extract_keywords(query)
        find_text = await self._cross_doc_find(cards, keywords)

        # Build intent context
        full_intent = f"\nQuery intent: {intent_context}" if intent_context else ""

        system, user = orchestrator_analysis(OrchestratorAnalysisParams(
            query=query,
            doc_cards=doc_cards_text,
            find_results=find_text,
            intent_context=full_intent,
        ))

        try:
            analysis_output = await llm.complete(system, user)
        except Exception as e:
            logger.error("Orchestrator analysis LLM call failed: %s", e)
            return []

        state.total_llm_calls += 1

        dispatches = parse_dispatch_plan(analysis_output, len(cards))

        if dispatches is None:
            logger.info("Analysis indicates already answered")
            return None

        if not dispatches:
            logger.info("No relevant documents found")
            return []

        state.analyze_done = True
        return dispatches

    # -----------------------------------------------------------------------
    # Phase 2: Supervisor loop
    # -----------------------------------------------------------------------

    async def _supervisor_loop(
        self,
        query: str,
        initial_dispatches: list[DispatchEntry],
        cards: list[DocCard],
        llm: LLMClient,
        state: _OrchestratorState,
    ) -> _SupervisorOutcome:
        """Run: dispatch → evaluate → replan loop."""
        current_dispatches = initial_dispatches
        iteration = 0
        eval_sufficient = False

        while iteration < MAX_SUPERVISOR_ITERATIONS:
            # Dispatch current plan
            if current_dispatches:
                await self._dispatch_and_collect(
                    query, current_dispatches, cards, llm, state,
                )

            # No evidence — nothing to evaluate
            if not state.all_evidence:
                logger.info("No evidence collected from any Worker")
                break

            # Skip evaluation for user-specified documents
            if self._skip_analysis:
                eval_sufficient = bool(state.all_evidence)
                break

            # Evaluate sufficiency
            try:
                eval_result = await evaluate(llm, query, state.all_evidence)
            except Exception as e:
                logger.error("Cross-doc evaluation failed: %s", e)
                break
            state.total_llm_calls += 1

            if eval_result.sufficient:
                eval_sufficient = True
                logger.info(
                    "Evidence sufficient (evidence=%d, iteration=%d)",
                    len(state.all_evidence), iteration,
                )
                break

            # Insufficient — replan
            logger.info(
                "Evidence insufficient (evidence=%d, iteration=%d) — replanning",
                len(state.all_evidence), iteration,
            )

            doc_cards_text = self._format_doc_cards(cards)
            try:
                new_dispatches = await self._replan(
                    query, eval_result.missing_info, state, cards, llm,
                )
            except Exception as e:
                logger.error("Replan failed: %s", e)
                break

            if not new_dispatches:
                logger.info("Replan produced no new dispatches — exiting")
                break

            current_dispatches = new_dispatches
            iteration += 1

        return _SupervisorOutcome(
            iteration=iteration,
            eval_sufficient=eval_sufficient,
            llm_calls=0,  # tracked in state.total_llm_calls
        )

    # -----------------------------------------------------------------------
    # Dispatch and collect
    # -----------------------------------------------------------------------

    async def _dispatch_and_collect(
        self,
        query: str,
        dispatches: list[DispatchEntry],
        cards: list[DocCard],
        llm: LLMClient,
        state: _OrchestratorState,
    ) -> None:
        """Dispatch Workers in parallel and collect results."""
        async def run_worker(dispatch: DispatchEntry) -> tuple[int, WorkerResult]:
            idx = dispatch.doc_idx
            if idx >= len(cards):
                logger.warning("Document index %d out of range, skipping", idx)
                return (idx, WorkerResult())

            card = cards[idx]

            await self._emit({
                "type": "worker_started",
                "doc_id": card.doc_id,
                "doc_name": card.name,
                "task": dispatch.task,
            })

            try:
                doc = await self._doc_loader(card.doc_id)
            except Exception as e:
                logger.warning("Failed to load document %s: %s", card.doc_id, e)
                await self._emit({
                    "type": "worker_error",
                    "doc_id": card.doc_id,
                    "error": str(e),
                })
                return (idx, WorkerResult())

            worker = Worker(
                document=doc,
                query=query,
                llm_client=llm,
                max_rounds=self._max_rounds,
                max_llm_calls=self._max_llm_calls,
                task=dispatch.task,
                intent_context=self._intent_context,
            )

            result = await worker.run()
            logger.info(
                "Worker completed for doc %d (%s): evidence=%d, rounds=%d",
                idx, card.name, len(result.evidence), result.rounds_used,
            )

            await self._emit({
                "type": "worker_done",
                "doc_id": card.doc_id,
                "doc_name": card.name,
                "evidence_count": len(result.evidence),
                "rounds_used": result.rounds_used,
            })

            return (idx, result)

        tasks = [run_worker(d) for d in dispatches]
        results = await asyncio.gather(*tasks, return_exceptions=True)

        for item in results:
            if isinstance(item, Exception):
                logger.warning("Worker failed: %s", item)
                continue
            idx, result = item
            state.collect_result(idx, result)

    # -----------------------------------------------------------------------
    # Replan
    # -----------------------------------------------------------------------

    async def _replan(
        self,
        query: str,
        missing_info: str,
        state: _OrchestratorState,
        cards: list[DocCard],
        llm: LLMClient,
    ) -> list[DispatchEntry]:
        """Replan dispatch targets based on missing information."""
        evidence_summary = _format_evidence_context(state.all_evidence)
        doc_cards_text = self._format_doc_cards(cards)

        system, user = orchestrator_replan_prompt(
            query=query,
            missing_info=missing_info,
            evidence_summary=evidence_summary,
            dispatched_indices=state.dispatched,
            doc_cards=doc_cards_text,
        )

        try:
            response = await llm.complete(system, user)
        except Exception as e:
            logger.error("Replan LLM call failed: %s", e)
            return []

        state.total_llm_calls += 1
        return parse_replan_response(response, len(cards), state.dispatched)

    # -----------------------------------------------------------------------
    # Helpers
    # -----------------------------------------------------------------------

    def _format_doc_cards(self, cards: list[DocCard]) -> str:
        """Format document cards for the analysis prompt."""
        lines = []
        for i, card in enumerate(cards, 1):
            concepts = f" (concepts: {', '.join(card.concepts[:5])})" if card.concepts else ""
            lines.append(
                f"[{i}] {card.name} — {card.summary} "
                f"({card.section_count} sections){concepts}"
            )
        return "\n".join(lines)

    async def _cross_doc_find(self, cards: list[DocCard], keywords: list[str]) -> str:
        """Search for keywords across all documents."""
        if not keywords:
            return "(no keywords extracted)"

        results = []
        for i, card in enumerate(cards):
            try:
                doc = await self._doc_loader(card.doc_id)
                for kw in keywords[:5]:
                    entries = await doc.keyword_entries(kw)
                    if entries:
                        titles = [
                            f"{await doc.node_title(e.node_id)} (weight {e.weight:.2f})"
                            for e in entries[:3]
                        ]
                        results.append(f"doc {i + 1}: keyword '{kw}' → {', '.join(titles)}")
            except Exception:
                pass

        return "\n".join(results) if results else "(no cross-document matches)"


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

@dataclass
class _SupervisorOutcome:
    iteration: int
    eval_sufficient: bool
    llm_calls: int


def _compute_confidence(
    eval_sufficient: bool,
    replan_rounds: int,
    no_evidence: bool,
) -> float:
    """Compute confidence from evaluation outcome."""
    if no_evidence:
        return 0.0
    if eval_sufficient:
        return max(0.5, 0.95 - replan_rounds * 0.15)
    return max(0.1, 0.4 - replan_rounds * 0.1)


def _extract_keywords(query: str) -> list[str]:
    """Extract simple keywords from a query."""
    stop_words = {
        "what", "is", "the", "a", "an", "how", "does", "do", "are",
        "in", "on", "at", "to", "for", "of", "with", "and", "or",
        "this", "that", "it", "from", "by", "was", "were", "be",
        "can", "could", "would", "should", "will", "has", "have",
        "had", "not", "but", "if", "then", "than", "so", "as",
    }
    words = re.findall(r"\b\w+\b", query.lower())
    return [w for w in words if w not in stop_words and len(w) > 2]


def _format_evidence_context(evidence: list[WorkerEvidence]) -> str:
    """Format collected evidence for the replan prompt."""
    if not evidence:
        return "(no evidence collected)"
    return "\n\n".join(
        f"[{e.title}] (from {e.source_path})\n{e.content}"
        for e in evidence
    )
