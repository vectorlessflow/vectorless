"""Orchestrator — the lead agent for multi-document retrieval.

A deliberately small lead + subagent design:

    run()
      1. analyze   — pick relevant documents + per-doc sub-task (≤1 LLM call;
                     skipped entirely when the user specified the documents)
      2. dispatch  — run one Scout per document, in parallel
      3. finalize  — dedup evidence (0 LLM) + ONE synthesis call that also
                     verifies sufficiency and returns a confidence
      4. (optional) one gap re-dispatch if the synthesis says evidence is
                     insufficient, then finalize once more

That's it. No blackboard, no nested replan, no multi-stage supervisor loop —
the per-document reasoning lives in the Scout (see ``scout.py``), and answer
grounding is folded into a single synthesis-verify step.
"""

from __future__ import annotations

import asyncio
import logging
from typing import Any

from pydantic import BaseModel, Field

from vectorless.ask.protocols import DocLoader, EventCallback
from vectorless.ask.events import AskEvent
from vectorless.ask.errors import LLMFailureError
from vectorless.ask.utils import extract_keywords, format_evidence
from vectorless.ask.types import (
    DispatchEntry,
    DocCard,
    Evidence,
    Output,
    OrchestratorState,
    WorkerOutput,
)
from vectorless.ask.scout import Scout
from vectorless.ask.route_cache import RouteCache
from vectorless.llm_client import LLMClient
from vectorless.ask.reasoning.types import QueryAnalysis, QueryIntent
from vectorless.ask.prompts import (
    OrchestratorAnalysisParams,
    orchestrator_analysis,
    parse_dispatch_plan,
)
from vectorless.rerank.synthesize import process as rerank_process

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Final synthesis-with-verify result (one structured LLM call)
# ---------------------------------------------------------------------------

class _FinalAnswer(BaseModel):
    """The lead's answer over the collected evidence, with a built-in sufficiency check."""

    answer: str = Field(description="The answer to the question, grounded in and citing the evidence sections.")
    sufficient: bool = Field(default=True, description="True if the evidence fully answers the question.")
    missing: str = Field(default="", description="If not sufficient, what specific information is still missing.")
    confidence: float = Field(default=0.7, description="Confidence in the answer, 0.0–1.0.")


class Orchestrator:
    """Coordinates multi-document retrieval with Scouts. ``run()`` returns an Output."""

    def __init__(
        self,
        query: str,
        doc_cards: list[DocCard],
        doc_loader: DocLoader,
        llm_client: LLMClient,
        *,
        skip_analysis: bool = False,
        query_analysis: QueryAnalysis | None = None,
        max_rounds: int = 15,
        max_llm_calls: int = 0,
        max_concurrent_workers: int = 5,
        event_callback: EventCallback | None = None,
    ) -> None:
        self._query = query
        self._doc_cards = doc_cards
        self._doc_loader = doc_loader
        self._llm = llm_client
        self._skip_analysis = skip_analysis
        self._query_analysis = query_analysis
        self._max_rounds = max_rounds
        self._max_llm_calls = max_llm_calls
        self._semaphore = asyncio.Semaphore(max_concurrent_workers)
        self._emit = event_callback or _noop_emit

    # -----------------------------------------------------------------------

    async def run(self) -> Output:
        state = OrchestratorState()
        intent_context = self._query_analysis.intent_context() if self._query_analysis else ""
        scout_context = self._scout_intent_context()
        intent_value = self._query_analysis.intent.value if self._query_analysis else "factual"
        route_cache = RouteCache.default()

        logger.info(
            "Orchestrator starting (docs=%d, skip_analysis=%s)",
            len(self._doc_cards), self._skip_analysis,
        )

        # 1. analyze — choose documents + per-doc sub-task
        dispatches = await self._analyze(state, intent_context)
        if not dispatches:
            await self._emit({"event": AskEvent.COMPLETED, "reason": "no_results"})
            return state.into_output("")
        await self._emit({"event": AskEvent.QUERY_ANALYZED, "dispatches": len(dispatches)})

        # 2. dispatch — one Scout per document, in parallel
        await self._dispatch(dispatches, state, scout_context, intent_value, route_cache, use_cache=True)
        if not state.all_evidence:
            await self._emit({"event": AskEvent.COMPLETED, "reason": "no_evidence"})
            return state.into_output("")
        await self._emit({
            "event": AskEvent.EVIDENCE_COLLECTED,
            "evidence_count": len(state.all_evidence),
        })

        # 3. finalize — dedup + one synthesis-with-verify call
        output, sufficient, missing = await self._finalize(state)

        # 4. one bounded gap re-dispatch (workspace queries only)
        if not sufficient and missing and not self._skip_analysis and state.dispatched:
            await self._emit({
                "event": AskEvent.REPLAN_TRIGGERED,
                "evidence_count": len(state.all_evidence),
                "gaps": [missing],
            })
            redispatch = [
                DispatchEntry(doc_idx=idx, reason="gap", task=missing)
                for idx in list(state.dispatched)
            ]
            await self._dispatch(
                redispatch, state, scout_context, intent_value, route_cache, use_cache=False,
            )
            output, _, _ = await self._finalize(state)

        await self._emit({
            "event": AskEvent.COMPLETED,
            "confidence": output.confidence,
            "evidence_count": len(output.evidence),
        })
        return output

    # -----------------------------------------------------------------------
    # 1. Analyze
    # -----------------------------------------------------------------------

    async def _analyze(
        self, state: OrchestratorState, intent_context: str,
    ) -> list[DispatchEntry] | None:
        """Select documents and write a per-doc sub-task. ≤1 LLM call."""
        cards = self._doc_cards

        if self._skip_analysis:
            return [
                DispatchEntry(doc_idx=i, reason="User-specified document", task=self._query)
                for i in range(len(cards))
            ]

        find_text = await self._cross_doc_find(cards, extract_keywords(self._query))
        system, user = orchestrator_analysis(OrchestratorAnalysisParams(
            query=self._query,
            doc_cards=_format_doc_cards(cards),
            find_results=find_text,
            intent_context=intent_context,
        ))

        try:
            analysis_output = await self._llm.complete(system, user)
        except Exception as e:  # noqa: BLE001
            logger.error("Orchestrator analysis failed: %s", e)
            return None

        state.total_llm_calls += 1
        dispatches = parse_dispatch_plan(analysis_output, len(cards))
        if not dispatches:
            logger.info("Analysis selected no documents (or already answered)")
            return None
        state.analyze_done = True
        return dispatches

    async def _cross_doc_find(self, cards: list[DocCard], keywords: list[str]) -> str:
        """Cheap cross-document title search via the navigation index (0 LLM)."""
        results: list[str] = []
        for card in cards:
            try:
                doc = await self._doc_loader(card.doc_id)
            except Exception:
                continue
            for kw in keywords[:5]:
                try:
                    hits = await doc.find(kw)
                except Exception:
                    continue
                for hit in (hits or [])[:3]:
                    results.append(
                        f"[{card.name}] '{kw}' → {hit.title} "
                        f"(depth {hit.depth}, {hit.leaf_count} leaves)"
                    )
        return "\n".join(results) if results else "(no cross-document matches)"

    # -----------------------------------------------------------------------
    # 2. Dispatch — parallel Scouts
    # -----------------------------------------------------------------------

    async def _dispatch(
        self,
        dispatches: list[DispatchEntry],
        state: OrchestratorState,
        scout_context: str,
        intent_value: str,
        route_cache: RouteCache | None,
        use_cache: bool = True,
    ) -> None:
        cards = self._doc_cards
        await self._emit({
            "event": AskEvent.WORKERS_DISPATCHED,
            "worker_count": len(dispatches),
        })

        async def run_one(dispatch: DispatchEntry) -> tuple[int, WorkerOutput] | None:
            idx = dispatch.doc_idx
            if idx >= len(cards):
                return None
            card = cards[idx]
            try:
                doc = await self._doc_loader(card.doc_id)
            except Exception as e:  # noqa: BLE001
                logger.warning("Failed to load document %s: %s", card.doc_id, e)
                return None
            scout = Scout(
                document=doc,
                query=self._query,
                llm_client=self._llm,
                max_rounds=self._max_rounds,
                max_llm_calls=self._max_llm_calls,
                task=dispatch.task,
                intent_context=scout_context,
                doc_id=card.doc_id,
                intent=intent_value,
                route_cache=route_cache,
                use_cache=use_cache,
            )
            result = await scout.run()
            await self._emit({
                "event": AskEvent.WORKER_COMPLETED,
                "doc_idx": idx,
                "doc_name": card.name,
                "evidence_count": len(result.evidence),
                "rounds_used": result.metrics.rounds_used,
            })
            return (idx, result)

        collected: list[tuple[int, WorkerOutput]] = []

        async with asyncio.TaskGroup() as tg:
            async def guarded(d: DispatchEntry) -> None:
                async with self._semaphore:
                    try:
                        r = await run_one(d)
                    except Exception as e:  # noqa: BLE001
                        logger.warning("Scout failed: %s", e)
                        return
                    if r is not None:
                        collected.append(r)

            for d in dispatches:
                tg.create_task(guarded(d))

        for idx, output in collected:
            state.collect_result(idx, output)

    # -----------------------------------------------------------------------
    # 3. Finalize — dedup + one synthesis-with-verify call
    # -----------------------------------------------------------------------

    async def _finalize(self, state: OrchestratorState) -> tuple[Output, bool, str]:
        intent_enum = self._query_analysis.intent if self._query_analysis else QueryIntent.FACTUAL
        intent_value = intent_enum.value
        reranked = rerank_process(
            evidence=state.all_evidence,
            intent=intent_enum,
            confidence=0.0,
        )
        if not reranked.evidence:
            return state.into_output(""), True, ""

        final = await self._synthesize(reranked.evidence[:8], intent_value)
        state.total_llm_calls += 1

        output = state.into_output(final.answer)
        output.evidence = reranked.evidence
        output.confidence = max(0.0, min(1.0, final.confidence))
        logger.info(
            "Finalize: evidence=%d llm_calls=%d confidence=%.2f sufficient=%s",
            len(output.evidence), output.metrics.llm_calls, output.confidence, final.sufficient,
        )
        return output, final.sufficient, final.missing.strip()

    async def _synthesize(self, evidence: list[Evidence], intent_value: str) -> _FinalAnswer:
        """One structured LLM call: synthesize the answer and verify sufficiency."""
        system = (
            "You answer a question using ONLY the provided evidence from the user's documents. "
            "Cite the source sections. Do not invent facts. If the evidence does not fully answer "
            "the question, set sufficient=false and state precisely what is missing. Provide a "
            "calibrated confidence in [0,1]."
        )
        user = (
            f"Question: {self._query}\n"
            f"Query type: {intent_value}\n\n"
            f"Evidence:\n{format_evidence(evidence)}\n\n"
            "Synthesize the answer, judge whether the evidence is sufficient, and rate your confidence."
        )
        try:
            return await self._llm.complete_structured(system, user, _FinalAnswer)
        except Exception as e:  # noqa: BLE001 — fall back to raw evidence as the answer
            logger.warning("Synthesis failed, using formatted evidence: %s", e)
            return _FinalAnswer(
                answer=format_evidence(evidence),
                sufficient=True,
                missing="",
                confidence=0.4,
            )

    # -----------------------------------------------------------------------
    # Helpers
    # -----------------------------------------------------------------------

    def _scout_intent_context(self) -> str:
        if not self._query_analysis:
            return ""
        try:
            return f"{self._query_analysis.intent.value} — {self._query_analysis.strategy.strategy_type}"
        except Exception:
            return self._query_analysis.intent.value


# ---------------------------------------------------------------------------
# Module helpers
# ---------------------------------------------------------------------------

async def _noop_emit(event: dict) -> None:
    """No-op event emitter."""


def _format_doc_cards(cards: list[DocCard]) -> str:
    lines = []
    for i, card in enumerate(cards, 1):
        concepts = f" (concepts: {', '.join(card.concepts[:5])})" if card.concepts else ""
        lines.append(
            f"[{i}] {card.name} — {card.summary} ({card.section_count} sections){concepts}"
        )
    return "\n".join(lines)
