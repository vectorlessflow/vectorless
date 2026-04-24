"""Orchestrator agent — coordinates multi-document retrieval.

Mirrors vectorless-core/vectorless-agent/src/orchestrator/.

The Orchestrator is always the entry point for retrieval:
- User specified doc_ids → skip_analysis=True → spawn Workers directly
- Workspace (unspecified) → analyze DocCards → select docs → spawn Workers

Both paths produce the same Output type and share the same finalize logic.

Flow:
    Orchestrator.run()
      Phase 1: analyze() → AnalyzeOutcome (dispatches or early return)
      Phase 2: supervisor loop → dispatch Workers → verify → replan
      Phase 3: finalize_output() → rerank → Output
"""

from __future__ import annotations

import asyncio
import logging
from dataclasses import dataclass
from typing import Any

from vectorless.ask.protocols import DocLoader, EventCallback
from vectorless.ask.utils import extract_keywords, format_evidence
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
from vectorless.ask.reasoning.types import QueryAnalysis
from vectorless.ask.reasoning.analyzer import QueryAnalyzer
from vectorless.ask.verify import VerifyPipeline, VerificationResult
from vectorless.ask.blackboard import SharedBlackboard, extract_discoveries
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
MAX_VERIFICATION_ITERATIONS = 2


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
    verification_result: VerificationResult | None = None


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
            query_analysis=analysis,
        )
        output = await orch.run()
    """

    def __init__(
        self,
        query: str,
        doc_cards: list[DocCard],
        doc_loader: DocLoader,
        llm_client: LLMClient,
        *,
        skip_analysis: bool = False,
        query_plan: Any = None,      # Deprecated: kept for backward compat
        query_analysis: QueryAnalysis | None = None,
        max_rounds: int = 15,
        max_llm_calls: int = 0,
        event_callback: EventCallback | None = None,
    ) -> None:
        self._query = query
        self._doc_cards = doc_cards
        self._doc_loader = doc_loader
        self._llm = llm_client
        self._skip_analysis = skip_analysis
        self._max_rounds = max_rounds
        self._max_llm_calls = max_llm_calls
        self._emit = event_callback or _noop_emit

        # Accept both old QueryPlan and new QueryAnalysis
        if query_analysis is not None:
            self._query_analysis = query_analysis
        elif query_plan is not None:
            # Backward compat: convert QueryPlan to QueryAnalysis
            self._query_analysis = query_plan.to_query_analysis()
        else:
            self._query_analysis = None

    async def run(self) -> Output:
        """Execute the Orchestrator: analyze → supervisor loop → finalize.

        Returns the final Output with answer, evidence, metrics, and trace.
        """
        query = self._query
        cards = self._doc_cards
        llm = self._llm
        state = OrchestratorState()

        intent_context = ""
        if self._query_analysis:
            intent_context = self._query_analysis.intent_context()

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

        state.total_llm_calls += analyze_result.llm_calls
        initial_dispatches = analyze_result.dispatches

        # --- Phase 2: Supervisor loop ---
        outcome = await self._supervisor_loop(
            query, initial_dispatches, cards, llm, state,
        )
        state.total_llm_calls += outcome.llm_calls

        # Use verification confidence if available, otherwise compute from eval
        if outcome.verification_result is not None:
            confidence = outcome.verification_result.overall_confidence
        else:
            confidence = _compute_confidence(
                eval_sufficient=outcome.eval_sufficient,
                replan_rounds=outcome.iteration,
                no_evidence=not state.all_evidence,
            )

        # --- Phase 3: Finalize — rerank + assemble Output ---
        if state.all_evidence:
            return await self._finalize_output(
                state,
                self._query_analysis.intent if self._query_analysis else None,
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
        keywords = extract_keywords(query)
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
        """Run: dispatch → verify → re-analyze → replan loop.

        Integrates SharedBlackboard for cross-Worker discovery sharing
        and VerifyPipeline for multi-dimensional evidence verification.
        """
        current_dispatches = initial_dispatches
        iteration = 0
        eval_sufficient = False
        llm_calls = 0
        verification_result: VerificationResult | None = None

        # Initialize shared blackboard for multi-doc coordination
        blackboard = SharedBlackboard()
        verify_pipeline = VerifyPipeline()

        while iteration < MAX_SUPERVISOR_ITERATIONS:
            # Dispatch current plan with adaptive strategy
            if current_dispatches:
                logger.info(
                    "Dispatching %d Workers (iteration=%d)",
                    len(current_dispatches), iteration,
                )
                await self._adaptive_dispatch(
                    query, current_dispatches, cards, llm, state,
                    blackboard, iteration,
                )

            # No evidence — nothing to verify
            if not state.all_evidence:
                logger.info("No evidence collected from any Worker")
                break

            # Skip verification for user-specified documents (no replan needed)
            if self._skip_analysis:
                eval_sufficient = bool(state.all_evidence)
                break

            # Verify evidence using multi-dimensional pipeline
            query_intent = ""
            if self._query_analysis:
                query_intent = self._query_analysis.intent.value

            try:
                verification_result = await verify_pipeline.verify(
                    query=query,
                    evidence=state.all_evidence,
                    query_intent=query_intent,
                    iteration=iteration,
                    llm=llm,
                )
                llm_calls += 1
            except Exception as e:
                logger.error("Verification failed: %s", e)
                break

            logger.info(
                "Verification result: passed=%s, confidence=%.2f, gaps=%d",
                verification_result.passed,
                verification_result.overall_confidence,
                len(verification_result.gaps),
            )

            if verification_result.passed:
                eval_sufficient = True
                break

            # Verification failed — check iteration limit
            if iteration >= MAX_VERIFICATION_ITERATIONS - 1:
                logger.info(
                    "Max verification iterations reached — returning with current confidence"
                )
                break

            # Re-analyze with gap context
            if self._query_analysis and verification_result.gaps:
                evidence_summary = format_evidence(state.all_evidence)
                analyzer = QueryAnalyzer()
                try:
                    self._query_analysis = await analyzer.re_analyze(
                        analysis=self._query_analysis,
                        gaps=verification_result.gaps,
                        evidence_summary=evidence_summary,
                        llm=llm,
                    )
                    llm_calls += 1
                except Exception as e:
                    logger.warning("Re-analysis failed: %s", e)

            # Replan with blackboard context
            logger.info(
                "Evidence insufficient (evidence=%d, iteration=%d) — replanning",
                len(state.all_evidence), iteration,
            )

            missing_info = "; ".join(verification_result.gaps) if verification_result.gaps else ""
            try:
                new_dispatches = await self._replan(
                    query, missing_info, state, cards, llm, blackboard,
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
            verification_result=verification_result,
        )

    # -----------------------------------------------------------------------
    # Adaptive dispatch — sequential or parallel based on iteration and doc count
    # -----------------------------------------------------------------------

    async def _adaptive_dispatch(
        self,
        query: str,
        dispatches: list[DispatchEntry],
        cards: list[DocCard],
        llm: LLMClient,
        state: OrchestratorState,
        blackboard: SharedBlackboard,
        iteration: int,
    ) -> None:
        """Dispatch Workers with adaptive strategy.

        - 1 document: parallel (no blackboard benefit)
        - 2+ documents, first iteration: sequential (build blackboard)
        - 2+ documents, subsequent iterations: parallel (blackboard pre-populated)
        """
        if len(dispatches) == 1:
            # Single doc: parallel (no blackboard benefit)
            await self._dispatch_parallel(
                query, dispatches, cards, llm, state, blackboard, "",
            )
        elif iteration == 0:
            # First iteration: sequential to build blackboard
            await self._dispatch_sequential(
                query, dispatches, cards, llm, state, blackboard,
            )
        else:
            # Subsequent iterations: parallel with full blackboard
            shared_context = blackboard.format_for_all()
            await self._dispatch_parallel(
                query, dispatches, cards, llm, state, blackboard, shared_context,
            )

    async def _dispatch_parallel(
        self,
        query: str,
        dispatches: list[DispatchEntry],
        cards: list[DocCard],
        llm: LLMClient,
        state: OrchestratorState,
        blackboard: SharedBlackboard,
        shared_context: str,
    ) -> None:
        """Dispatch Workers in parallel and collect results."""
        intent_context = ""
        if self._query_analysis:
            intent_context = f"{self._query_analysis.intent.value} — {self._query_analysis.strategy.strategy_type}"

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
                shared_context=shared_context,
            )

            result = await worker.run()
            logger.info(
                "Worker completed for doc %d (%s): evidence=%d, rounds=%d",
                idx, card.name, len(result.evidence), result.metrics.rounds_used,
            )
            return (idx, result)

        tasks = [run_worker(d) for d in dispatches]

        # Use TaskGroup with per-task exception wrapping to maintain
        # the same fault-tolerance as gather(return_exceptions=True)
        task_results: list[tuple[int, WorkerOutput] | Exception] = []
        async with asyncio.TaskGroup() as tg:
            async def _safe_run(d: DispatchEntry) -> None:
                try:
                    result = await run_worker(d)
                    task_results.append(result)
                except Exception as e:
                    task_results.append(e)
                    logger.warning("Worker failed: %s", e)

            for d in dispatches:
                tg.create_task(_safe_run(d))

        for item in task_results:
            if isinstance(item, Exception):
                continue
            idx, output = item
            state.collect_result(idx, output)
            # Extract discoveries to blackboard
            card = cards[idx] if idx < len(cards) else None
            if card:
                discoveries = extract_discoveries(output, card.name)
                for d in discoveries:
                    blackboard.add_discovery(d)

    async def _dispatch_sequential(
        self,
        query: str,
        dispatches: list[DispatchEntry],
        cards: list[DocCard],
        llm: LLMClient,
        state: OrchestratorState,
        blackboard: SharedBlackboard,
    ) -> None:
        """Dispatch Workers sequentially to build blackboard context."""
        intent_context = ""
        if self._query_analysis:
            intent_context = f"{self._query_analysis.intent.value} — {self._query_analysis.strategy.strategy_type}"

        for dispatch in dispatches:
            idx = dispatch.doc_idx
            if idx >= len(cards):
                logger.warning("Document index %d out of range, skipping", idx)
                continue

            card = cards[idx]

            try:
                doc = await self._doc_loader(card.doc_id)
            except Exception as e:
                logger.warning("Failed to load document %s: %s", card.doc_id, e)
                continue

            # Get context from blackboard for this Worker
            shared_context = blackboard.format_for_worker(card.name)

            worker = Worker(
                document=doc,
                query=query,
                llm_client=llm,
                max_rounds=self._max_rounds,
                max_llm_calls=self._max_llm_calls,
                task=dispatch.task,
                intent_context=intent_context,
                shared_context=shared_context,
            )

            result = await worker.run()
            logger.info(
                "Worker completed for doc %d (%s): evidence=%d, rounds=%d",
                idx, card.name, len(result.evidence), result.metrics.rounds_used,
            )

            state.collect_result(idx, result)

            # Extract discoveries to blackboard for subsequent Workers
            discoveries = extract_discoveries(result, card.name)
            for d in discoveries:
                blackboard.add_discovery(d)

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
        blackboard: SharedBlackboard | None = None,
    ) -> list[DispatchEntry]:
        """Replan dispatch targets based on missing information."""
        evidence_summary = format_evidence(state.all_evidence)
        doc_cards_text = _format_doc_cards(cards)

        # Include blackboard context in replan
        keywords_text = ""
        if blackboard and blackboard.active_leads:
            keywords_text = "\n\nActive leads from other Workers:\n" + "\n".join(
                f"- {lead}" for lead in blackboard.active_leads[:5]
            )
        if blackboard and blackboard.cross_references:
            cross_refs = []
            for src, targets in blackboard.cross_references.items():
                cross_refs.append(f"  {src} → {', '.join(targets)}")
            if cross_refs:
                keywords_text += "\n\nCross-document references:\n" + "\n".join(cross_refs)

        system, user = orchestrator_replan_prompt(
            query=query,
            missing_info=missing_info,
            evidence_summary=evidence_summary,
            dispatched_indices=state.dispatched,
            doc_cards=doc_cards_text,
            keywords_text=keywords_text,
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
        state: OrchestratorState,
        intent: Any,  # QueryIntent or None
        confidence: float,
    ) -> Output:
        """Rerank evidence and assemble the final Output."""
        from vectorless.ask.plan import QueryIntent as PlanIntent
        from vectorless.ask.reasoning.types import QueryIntent

        # Map reasoning QueryIntent to plan QueryIntent for rerank compat
        if intent is not None:
            intent_value = intent.value if hasattr(intent, "value") else str(intent)
            _plan_intent_map = {
                "factual": PlanIntent.FACTUAL,
                "analytical": PlanIntent.ANALYTICAL,
                "navigational": PlanIntent.NAVIGATIONAL,
                "summary": PlanIntent.SUMMARY,
                "comparative": PlanIntent.ANALYTICAL,
                "procedural": PlanIntent.FACTUAL,
            }
            effective_intent = _plan_intent_map.get(intent_value, PlanIntent.FACTUAL)
        else:
            effective_intent = PlanIntent.FACTUAL

        reranked = rerank_process(
            evidence=state.all_evidence,
            intent=effective_intent,
            confidence=confidence,
        )

        state.total_llm_calls += reranked.llm_calls

        output = state.into_output(reranked.answer)
        output.confidence = reranked.confidence

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
