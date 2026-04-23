"""Orchestrator agent — coordinates multi-document retrieval.

Mirrors vectorless-core/vectorless-agent/src/orchestrator/.

The Orchestrator is always the entry point for retrieval:
- User specified doc_ids → skip_analysis=True → spawn Workers directly
- Workspace (unspecified) → analyze DocCards → select docs → spawn Workers

Both paths produce the same Output type and share the same finalize logic.

Flow:
    Orchestrator.run()
      Phase 1: analyze() → AnalyzeOutcome (dispatches or early return)
      Phase 2: supervisor loop → dispatch Workers → evaluate → replan
      Phase 3: finalize_output() → rerank → Output
"""

from __future__ import annotations

import asyncio
import logging
import re
from dataclasses import dataclass
from typing import Any

from vectorless.ask.types import (
    DispatchEntry,
    DocCard,
    Evidence,
    EvalResult,
    Metrics,
    OrchestratorState,
    Output,
    WorkerOutput,
)
from vectorless.ask.evaluate import evaluate
from vectorless.ask.worker import Worker
from vectorless.llm_client import LLMClient
from vectorless.ask.plan import QueryPlan
from vectorless.ask.prompts import (
    OrchestratorAnalysisParams,
    orchestrator_analysis,
    orchestrator_replan_prompt,
    parse_dispatch_plan,
    parse_replan_response,
)
from vectorless.rerank.synthesize import RerankOutput, process as rerank_process

logger = logging.getLogger(__name__)

MAX_SUPERVISOR_ITERATIONS = 3


# ---------------------------------------------------------------------------
# Analyze outcome — mirrors Rust AnalyzeOutcome
# ---------------------------------------------------------------------------

@dataclass
class _AnalyzeOutcome:
    """Result of the analyze phase."""
    dispatches: list[DispatchEntry]
    llm_calls: int


# ---------------------------------------------------------------------------
# Supervisor outcome — mirrors Rust SupervisorOutcome
# ---------------------------------------------------------------------------

@dataclass
class _SupervisorOutcome:
    """Outcome of the supervisor loop."""
    iteration: int
    eval_sufficient: bool
    llm_calls: int


# ---------------------------------------------------------------------------
# Orchestrator — mirrors Rust orchestrator/mod.rs
# ---------------------------------------------------------------------------

class Orchestrator:
    """Coordinates multi-document retrieval with Workers.

    Holds all execution context. Calling run() produces an Output.

    Usage::

        orch = Orchestrator(
            query="Compare revenue across years",
            doc_cards=[card1, card2],
            doc_loader=load_fn,
            llm_client=llm,
            query_plan=plan,
        )
        output = await orch.run()
    """

    def __init__(
        self,
        query: str,
        doc_cards: list[DocCard],
        doc_loader: Any,  # async callable: (doc_id: str) -> PyDocument
        llm_client: LLMClient,
        *,
        skip_analysis: bool = False,
        query_plan: QueryPlan | None = None,
        max_rounds: int = 15,
        max_llm_calls: int = 0,
        event_callback: Any = None,  # async callable: (dict) -> None
    ) -> None:
        self._query = query
        self._doc_cards = doc_cards
        self._doc_loader = doc_loader
        self._llm = llm_client
        self._skip_analysis = skip_analysis
        self._query_plan = query_plan
        self._max_rounds = max_rounds
        self._max_llm_calls = max_llm_calls
        self._emit = event_callback or _noop_emit

    async def run(self) -> Output:
        """Execute the Orchestrator: analyze → supervisor loop → finalize.

        Returns the final Output with answer, evidence, metrics, and trace.
        """
        query = self._query
        cards = self._doc_cards
        llm = self._llm
        state = OrchestratorState()
        orch_llm_calls: int = 0

        intent_context = ""
        if self._query_plan:
            intent_context = self._query_plan.intent_context()

        logger.info(
            "Orchestrator starting (docs=%d, skip_analysis=%s)",
            len(cards), self._skip_analysis,
        )

        # --- Phase 1: Analyze ---
        analyze_result = await self._analyze(
            query, cards, llm, state, self._skip_analysis, intent_context,
        )

        if analyze_result is None:
            # No results or already answered
            return state.into_output("")

        orch_llm_calls += analyze_result.llm_calls
        initial_dispatches = analyze_result.dispatches

        # --- Phase 2: Supervisor loop ---
        outcome = await self._supervisor_loop(
            query, initial_dispatches, cards, llm, state,
        )
        orch_llm_calls += outcome.llm_calls

        confidence = _compute_confidence(
            eval_sufficient=outcome.eval_sufficient,
            replan_rounds=outcome.iteration,
            no_evidence=not state.all_evidence,
        )

        # --- Phase 3: Finalize — rerank + assemble Output ---
        if state.all_evidence:
            multi_doc = len(cards) > 1
            return await self._finalize_output(
                query, state, orch_llm_calls, multi_doc,
                self._query_plan.intent if self._query_plan else None,
                confidence,
            )

        logger.info("No evidence collected — returning empty output")
        return state.into_output("")

    # -----------------------------------------------------------------------
    # Phase 1: Analyze — mirrors Rust orchestrator/analyze.rs
    # -----------------------------------------------------------------------

    async def _analyze(
        self,
        query: str,
        cards: list[DocCard],
        llm: LLMClient,
        state: OrchestratorState,
        skip_analysis: bool,
        intent_context: str,
    ) -> _AnalyzeOutcome | None:
        """Analyze documents and produce a dispatch plan.

        Returns None if no results / already answered, or _AnalyzeOutcome.
        """
        if skip_analysis:
            logger.debug("Phase 1: skipping (user-specified documents)")
            return _AnalyzeOutcome(
                dispatches=[
                    DispatchEntry(
                        doc_idx=i,
                        reason="User-specified document",
                        task=query,
                    )
                    for i in range(len(cards))
                ],
                llm_calls=0,
            )

        # Build doc cards text
        doc_cards_text = _format_doc_cards(cards)

        # Cross-document keyword search
        keywords = _extract_keywords(query)
        find_text = await self._cross_doc_find(cards, keywords)

        # Build analysis prompt with query understanding context
        system, user = orchestrator_analysis(OrchestratorAnalysisParams(
            query=query,
            doc_cards=doc_cards_text,
            find_results=find_text,
            intent_context=intent_context,
        ))

        try:
            analysis_output = await llm.complete(system, user)
        except Exception as e:
            logger.error("Orchestrator analysis LLM call failed: %s", e)
            return None

        logger.info(
            "Phase 1: analysis complete (response_len=%d)", len(analysis_output),
        )

        dispatches = parse_dispatch_plan(analysis_output, len(cards))

        if dispatches is None:
            logger.info("Analysis indicates already answered")
            return None

        if not dispatches:
            logger.info("No relevant documents found")
            return None

        state.analyze_done = True
        return _AnalyzeOutcome(dispatches=dispatches, llm_calls=1)

    # -----------------------------------------------------------------------
    # Cross-document search — mirrors Rust orchestrator cross-doc find
    # -----------------------------------------------------------------------

    async def _cross_doc_find(
        self,
        cards: list[DocCard],
        keywords: list[str],
    ) -> str:
        """Search across documents for keywords using navigation index."""
        results: list[str] = []
        for card in cards:
            try:
                doc = await self._doc_loader(card.doc_id)
            except Exception:
                continue
            for kw in keywords[:5]:
                try:
                    hits = await doc.find(kw)
                    if hits:
                        for hit in hits[:3]:
                            results.append(
                                f"[{card.name}] '{kw}' → {hit.title} "
                                f"(depth {hit.depth}, {hit.leaf_count} leaves)"
                            )
                except Exception:
                    pass
        return "\n".join(results) if results else "(no cross-document matches)"

    # -----------------------------------------------------------------------
    # Phase 2: Supervisor loop — mirrors Rust orchestrator/supervisor.rs
    # -----------------------------------------------------------------------

    async def _supervisor_loop(
        self,
        query: str,
        initial_dispatches: list[DispatchEntry],
        cards: list[DocCard],
        llm: LLMClient,
        state: OrchestratorState,
    ) -> _SupervisorOutcome:
        """Run: dispatch → evaluate → replan loop."""
        current_dispatches = initial_dispatches
        iteration = 0
        eval_sufficient = False
        llm_calls = 0

        while iteration < MAX_SUPERVISOR_ITERATIONS:
            # Dispatch current plan
            if current_dispatches:
                logger.info(
                    "Dispatching %d Workers (iteration=%d)",
                    len(current_dispatches), iteration,
                )
                await self._dispatch_and_collect(
                    query, current_dispatches, cards, llm, state,
                )

            # No evidence — nothing to evaluate
            if not state.all_evidence:
                logger.info("No evidence collected from any Worker")
                break

            # Skip evaluation for user-specified documents (no replan needed)
            if self._skip_analysis:
                eval_sufficient = bool(state.all_evidence)
                break

            # Evaluate sufficiency
            try:
                eval_result = await evaluate(llm, query, state.all_evidence)
            except Exception as e:
                logger.error("Cross-doc evaluation failed: %s", e)
                break
            llm_calls += 1

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
            llm_calls=llm_calls,
        )

    # -----------------------------------------------------------------------
    # Dispatch and collect — mirrors Rust orchestrator/dispatch.rs
    # -----------------------------------------------------------------------

    async def _dispatch_and_collect(
        self,
        query: str,
        dispatches: list[DispatchEntry],
        cards: list[DocCard],
        llm: LLMClient,
        state: OrchestratorState,
    ) -> None:
        """Dispatch Workers in parallel and collect results."""
        intent_context = ""
        if self._query_plan:
            intent_context = f"{self._query_plan.intent.value} — {self._query_plan.strategy_hint}"

        async def run_worker(dispatch: DispatchEntry) -> tuple[int, WorkerOutput]:
            idx = dispatch.doc_idx
            if idx >= len(cards):
                logger.warning("Document index %d out of range, skipping", idx)
                return (idx, WorkerOutput())

            card = cards[idx]

            try:
                doc = await self._doc_loader(card.doc_id)
            except Exception as e:
                logger.warning("Failed to load document %s: %s", card.doc_id, e)
                return (idx, WorkerOutput())

            worker = Worker(
                document=doc,
                query=query,
                llm_client=llm,
                max_rounds=self._max_rounds,
                max_llm_calls=self._max_llm_calls,
                task=dispatch.task,
                intent_context=intent_context,
            )

            result = await worker.run()
            logger.info(
                "Worker completed for doc %d (%s): evidence=%d, rounds=%d",
                idx, card.name, len(result.evidence), result.metrics.rounds_used,
            )
            return (idx, result)

        tasks = [run_worker(d) for d in dispatches]
        results = await asyncio.gather(*tasks, return_exceptions=True)

        for item in results:
            if isinstance(item, Exception):
                logger.warning("Worker failed: %s", item)
                continue
            idx, output = item
            state.collect_result(idx, output)

    # -----------------------------------------------------------------------
    # Replan — mirrors Rust orchestrator/replan.rs
    # -----------------------------------------------------------------------

    async def _replan(
        self,
        query: str,
        missing_info: str,
        state: OrchestratorState,
        cards: list[DocCard],
        llm: LLMClient,
    ) -> list[DispatchEntry]:
        """Replan dispatch targets based on missing information."""
        evidence_summary = _format_evidence_context(state.all_evidence)
        doc_cards_text = _format_doc_cards(cards)

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
    # Finalize — mirrors Rust orchestrator::finalize_output
    # -----------------------------------------------------------------------

    async def _finalize_output(
        self,
        query: str,
        state: OrchestratorState,
        orch_llm_calls: int,
        multi_doc: bool,
        intent: Any,  # QueryIntent or None
        confidence: float,
    ) -> Output:
        """Rerank evidence and assemble the final Output."""
        from vectorless.ask.plan import QueryIntent

        effective_intent = intent or QueryIntent.FACTUAL

        reranked = rerank_process(
            evidence=state.all_evidence,
            intent=effective_intent,
            confidence=confidence,
        )

        total_llm_calls = orch_llm_calls + reranked.llm_calls

        output = state.into_output(reranked.answer)
        output.confidence = reranked.confidence
        output.metrics.llm_calls += total_llm_calls

        logger.info(
            "Orchestrator complete (evidence=%d, llm_calls=%d, confidence=%.2f)",
            len(output.evidence), output.metrics.llm_calls, output.confidence,
        )

        return output


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _noop_emit(event: dict) -> Any:
    """No-op event emitter."""
    return asyncio.ensure_future(asyncio.sleep(0))


def _compute_confidence(
    eval_sufficient: bool,
    replan_rounds: int,
    no_evidence: bool,
) -> float:
    """Compute confidence from LLM evaluate() outcome.

    Mirrors Rust compute_confidence.
    """
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


def _format_doc_cards(cards: list[DocCard]) -> str:
    """Format document cards for the analysis prompt."""
    lines = []
    for i, card in enumerate(cards, 1):
        concepts = f" (concepts: {', '.join(card.concepts[:5])})" if card.concepts else ""
        lines.append(
            f"[{i}] {card.name} — {card.summary} "
            f"({card.section_count} sections){concepts}"
        )
    return "\n".join(lines)


def _format_evidence_context(evidence: list[Evidence]) -> str:
    """Format collected evidence for the replan prompt."""
    if not evidence:
        return "(no evidence collected)"
    return "\n\n".join(
        f"[{e.node_title}] (from {e.doc_name or 'unknown'})\n{e.content}"
        for e in evidence
    )
