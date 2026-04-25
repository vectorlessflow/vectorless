"""Worker agent — navigates a single document to collect evidence.

The Worker uses an LLM-driven command loop:
1. Phase 0: Initial `ls` to observe the top-level structure
2. Phase 1.5 (optional): LLM generates a navigation plan from keyword hints
3. Phase 2: Main loop — LLM picks a command → execute → record trace → repeat
"""

from __future__ import annotations

import logging

from vectorless.ask.protocols import NavigableDocument
from vectorless.ask.errors import LLMFailureError
from vectorless.ask.types import TraceStep, WorkerOutput, WorkerState
from vectorless.ask.utils import extract_keywords
from vectorless.llm_client import LLMClient
from vectorless.ask.worker.parse import Command, parse_command, _is_parse_failure
from vectorless.ask.worker.commands import execute_command, _visited_titles, Step
from vectorless.ask.prompts import (
    NavigationParams,
    WorkerDispatchParams,
    build_plan_prompt,
    build_replan_prompt,
    worker_dispatch,
    worker_navigation,
)

logger = logging.getLogger(__name__)


class Worker:
    """Navigates a single document to collect evidence for a query.

    Usage::

        worker = Worker(document=doc, query="What is the revenue?", llm_client=llm)
        result = await worker.run()
    """

    def __init__(
        self,
        document: NavigableDocument,
        query: str,
        llm_client: LLMClient,
        *,
        max_rounds: int = 15,
        max_llm_calls: int = 0,
        task: str | None = None,
        intent_context: str = "",
        shared_context: str = "",
    ) -> None:
        self._doc = document
        self._query = query
        self._llm = llm_client
        self._max_rounds = max_rounds
        self._max_llm_calls = max_llm_calls
        self._task = task
        self._intent_context = intent_context
        self._shared_context = shared_context

    async def run(self) -> WorkerOutput:
        """Execute the Worker navigation loop and return collected evidence."""
        doc = self._doc
        query = self._query
        llm = self._llm
        task = self._task
        max_rounds = self._max_rounds
        max_llm = self._max_llm_calls
        intent_context = self._intent_context
        shared_context = self._shared_context

        state = WorkerState(remaining=max_rounds, max_rounds=max_rounds)

        # Phase 0: initial ls to observe environment
        root_id = await doc.root_id()
        state.visited.add(root_id)
        doc_name = ""
        try:
            doc_name = await doc.doc_name()
        except Exception:
            pass
        logger.info("Worker starting: doc=%s max_rounds=%d", doc_name, max_rounds)

        try:
            children = await doc.ls()
            if children:
                lines = []
                for i, child in enumerate(children, 1):
                    lines.append(
                        f"[{i}] {child.title} — "
                        f"(depth {child.depth}, {child.leaf_count} leaves)"
                    )
                state.last_feedback = "\n".join(lines)
            else:
                state.last_feedback = "(no children at root)"
        except Exception as e:
            state.last_feedback = f"Initial ls failed: {e}"

        # Phase 1.5: optional navigation planning
        keyword_hints = ""
        try:
            keyword_hints = await self._build_keyword_hints(doc, query)
        except Exception:
            pass

        if keyword_hints:
            logger.info("Phase 1.5: keyword hints available, generating plan")
            await self._generate_plan(doc, query, task, state, keyword_hints, llm)
            if state.plan:
                logger.info("Phase 1.5: plan generated — %s", state.plan[:150])

        # Phase 2: main navigation loop
        use_dispatch = task is not None

        while state.remaining > 0:
            if max_llm > 0 and state.llm_calls >= max_llm:
                logger.info("LLM call budget exhausted (%d/%d)", state.llm_calls, max_llm)
                break

            # Build prompt
            if use_dispatch and state.remaining == max_rounds:
                system, user = worker_dispatch(WorkerDispatchParams(
                    original_query=query,
                    task=task or query,
                    doc_name=await doc.doc_name(),
                    breadcrumb=state.path_str(),
                    shared_context=shared_context,
                ))
            else:
                visited_titles = await _visited_titles(state, doc)
                system, user = worker_navigation(NavigationParams(
                    query=query,
                    task=task,
                    breadcrumb=state.path_str(),
                    evidence_summary=state.evidence_summary(),
                    missing_info=state.missing_info,
                    last_feedback=state.last_feedback,
                    remaining=state.remaining,
                    max_rounds=state.max_rounds,
                    history=state.history_text(),
                    visited_titles=visited_titles,
                    plan=state.plan,
                    intent_context=intent_context,
                    keyword_hints=keyword_hints,
                    shared_context=shared_context,
                ))

            # LLM decision
            round_num = max_rounds - state.remaining + 1
            try:
                llm_output = await llm.complete(system, user)
            except LLMFailureError as e:
                logger.error("LLM call failed at round %d: %s", round_num, e)
                break
            except Exception as e:
                logger.error("Unexpected error at round %d: %s", round_num, e)
                break
            state.llm_calls += 1

            # Parse command
            command = parse_command(llm_output)
            is_failure = _is_parse_failure(command, llm_output)

            if is_failure:
                logger.warning("round %d: parse failure — %s", round_num, llm_output.strip()[:100])
                raw_preview = llm_output.strip()[:200]
                if len(llm_output.strip()) > 200:
                    raw_preview += "..."
                state.last_feedback = (
                    f"Your output was not recognized as a valid command:\n"
                    f'"{raw_preview}"\n\n'
                    f"Please output exactly one command "
                    f"(ls, cd, cat, head, find, grep, toc, stats, similar, overview, "
                    f"siblings, ancestors, doc_card, concepts, find_section, "
                    f"compare, trace, summarize, wc, pwd, check, or done)."
                )
                state.push_history("(unrecognized) \u2192 parse failure")
                continue

            is_check = command.kind == "check"

            # Execute via command registry
            step = await execute_command(command, doc, state, query, llm)

            # Log tool call and response
            cmd_str = command.kind
            if command.target:
                cmd_str += f" {command.target}"
            feedback_preview = state.last_feedback
            if len(feedback_preview) > 200:
                feedback_preview = feedback_preview[:200] + "..."
            logger.info(
                "round %d: %s → %s",
                round_num, cmd_str, feedback_preview,
            )

            # Re-plan after insufficient check
            if is_check:
                await self._handle_replan(query, task, doc, state, llm, max_llm)

            # Record history and trace
            cmd_str = command.kind
            if command.target:
                cmd_str += f" {command.target}"

            feedback_preview = state.last_feedback
            if len(feedback_preview) > 120:
                feedback_preview = feedback_preview[:120] + "..."
            state.push_history(f"{cmd_str} \u2192 {feedback_preview}")

            round_num_done = max_rounds - state.remaining
            state.trace_steps.append(TraceStep(
                action=cmd_str,
                observation=state.last_feedback[:200],
                round=round_num_done,
            ))

            # Check termination
            if step.kind == "done":
                break
            elif step.kind == "force_done":
                break
            else:
                if not is_check:
                    state.remaining -= 1

        doc_name = ""
        try:
            doc_name = await doc.doc_name()
        except Exception:
            pass

        logger.info(
            "Worker done: doc=%s rounds=%d/%d evidence=%d llm_calls=%d",
            doc_name, max_rounds - state.remaining, max_rounds,
            len(state.evidence), state.llm_calls,
        )

        return state.into_worker_output(doc_name)

    async def _build_keyword_hints(self, doc: NavigableDocument, query: str) -> str:
        """Build keyword hints from the document's reasoning index and acceleration data."""
        keywords = extract_keywords(query)

        if not keywords:
            return ""

        hints = []

        # Keyword index matches
        for kw in keywords[:5]:
            try:
                entries = await doc.keyword_entries(kw)
                for entry in entries[:3]:
                    title = await doc.node_title(entry.node_id)
                    hints.append(
                        f"  - '{kw}' → {title} (weight {entry.weight:.2f})"
                    )
            except Exception:
                pass

        # Concept routes (pre-computed by RoutePass)
        route_hints = []
        for kw in keywords[:5]:
            try:
                routes = await doc.concept_routes(kw)
                for route in routes[:1]:
                    for target in route.targets[:3]:
                        title = await doc.node_title(target.node_id)
                        route_hints.append(
                            f"  - [{route.concept}] {title} "
                            f"(relevance {target.relevance:.2f}: {target.reason})"
                        )
            except Exception:
                pass

        # Top evidence scores (pre-computed by ScorePass)
        score_hints = []
        try:
            scores = await doc.evidence_scores_ranked()
            for s in scores[:5]:
                title = await doc.node_title(s.node_id)
                score_hints.append(
                    f"  - {title} (score {s.composite:.2f}: "
                    f"density={s.density:.2f} richness={s.data_richness:.2f})"
                )
        except Exception:
            pass

        sections = []
        if hints:
            sections.append(
                "Keyword matches (use find <keyword> to jump directly):\n"
                + "\n".join(hints)
            )
        if route_hints:
            sections.append(
                "Pre-computed routes:\n" + "\n".join(route_hints)
            )
        if score_hints:
            sections.append(
                "High-value evidence nodes:\n" + "\n".join(score_hints)
            )

        if not sections:
            return ""

        return "\n\n".join(sections) + "\n"

    async def _generate_plan(
        self,
        doc: NavigableDocument,
        query: str,
        task: str | None,
        state: WorkerState,
        keyword_hints: str,
        llm: LLMClient,
    ) -> None:
        """Phase 1.5: generate a navigation plan from keyword hints."""
        ls_output = state.last_feedback
        doc_name = await doc.doc_name()

        system, user = build_plan_prompt(
            query=query,
            ls_output=ls_output,
            doc_name=doc_name,
            keyword_hints_section=f"\n{keyword_hints}" if keyword_hints else "",
            task=task,
        )

        try:
            plan = await llm.complete(system, user)
            state.llm_calls += 1
            plan_text = plan.strip()
            if plan_text:
                state.plan = plan_text
                state.plan_generated = True
        except Exception as e:
            logger.warning("Plan generation failed: %s", e)

    async def _handle_replan(
        self,
        query: str,
        task: str | None,
        doc: NavigableDocument,
        state: WorkerState,
        llm: LLMClient,
        max_llm: int,
    ) -> None:
        """Dynamic re-planning after an insufficient check."""
        if not state.missing_info:
            return

        if state.remaining < 3:
            state.plan = ""
            state.missing_info = ""
            return

        if max_llm > 0 and state.llm_calls >= max_llm:
            state.plan = ""
            state.missing_info = ""
            return

        # Build sibling hints
        sibling_hints = ""
        current_children = "Current position is a leaf node \u2014 consider cd .. to go back.\n"

        try:
            children = await doc.ls()
            if children:
                items = [f"  - {c.title} ({c.leaf_count} leaves)" for c in children]
                current_children = f"Children at current position:\n" + "\n".join(items) + "\n"
        except Exception:
            pass

        system, user = build_replan_prompt(
            query=query,
            task=task,
            path_str=state.path_str(),
            evidence_summary=state.evidence_summary(),
            missing_info=state.missing_info,
            visited_titles=await _visited_titles(state, doc),
            current_children=current_children,
            sibling_hints=sibling_hints,
            remaining=state.remaining,
            max_rounds=state.max_rounds,
        )

        try:
            new_plan = await llm.complete(system, user)
            state.llm_calls += 1
            plan_text = new_plan.strip()
            if plan_text:
                logger.info("Re-plan generated: %s", plan_text[:200])
                state.plan = plan_text
        except Exception as e:
            logger.warning("Re-plan LLM call failed: %s", e)

        state.missing_info = ""
