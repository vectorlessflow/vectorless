"""Prompt templates for the retrieval agent.

Extracted from vectorless-core/vectorless-agent/src/prompts.rs,
vectorless-core/vectorless-agent/src/worker/planning.rs, and
vectorless-core/vectorless-agent/src/orchestrator/replan.rs.

Templates:
1. worker_navigation   — Worker nav loop, every round
2. worker_dispatch     — Worker first round (when Orchestrator dispatches)
3. orchestrator_analysis — Orchestrator Phase 1
4. check_sufficiency   — evidence sufficiency evaluation
5. build_plan_prompt   — Phase 1.5 navigation planning
6. build_replan_prompt — Worker re-plan after insufficient check
7. orchestrator_replan — Orchestrator re-dispatch after insufficient evidence
"""

from __future__ import annotations

from dataclasses import dataclass


# ---------------------------------------------------------------------------
# 1. Worker Navigation (used every round in the nav loop)
# ---------------------------------------------------------------------------

@dataclass
class NavigationParams:
    query: str
    task: str | None = None
    breadcrumb: str = "root"
    evidence_summary: str = "(none)"
    missing_info: str = ""
    last_feedback: str = ""
    remaining: int = 15
    max_rounds: int = 15
    history: str = "(no history yet)"
    visited_titles: str = "(none)"
    plan: str = ""
    intent_context: str = ""
    keyword_hints: str = ""


_WORKER_NAVIGATION_SYSTEM = """\
You are a document navigation assistant. You navigate inside a document to find \
information that answers the user's question.

Available commands:
- ls                List children at current position (with summaries and leaf counts)
- cd <name>         Enter a child node (supports relative paths like Section/Sub and absolute paths like /root/Section)
- cd ..             Go back to parent node
- cat <name>        Read a child node's content (automatically collected as evidence)
- cat               Read the current node's content (useful at leaf nodes)
- head <name>       Preview first 20 lines of a node (does NOT collect evidence)
- find <keyword>    Search for a keyword in the document index (also supports multi-word like 'Lab C')
- findtree <pattern> Search for nodes by title pattern (case-insensitive)
- grep <pattern>    Regex search across all content in current subtree
- wc <name>         Show content size (lines, words, chars)
- pwd               Show current navigation path
- check             Evaluate if collected evidence is sufficient
- done              End navigation

SEARCH STRATEGY (important — follow this priority order):
- When keyword matches are shown below, navigate directly to the highest-weight matched node. \
Do NOT explore other branches first — the keyword index has already identified the most relevant location.
- When find results include content snippets that answer the question, cd to that node and cat it immediately.
- Use find with the EXACT keyword from the list (single word, \
not multi-word phrases). Example: if hint shows keyword 'performance' pointing to Performance section, \
use find performance, NOT find "performance guide".
- Use ls only when you have no keyword hints or need to discover the structure of an unknown section.
- Use findtree when you know a section title pattern but not the exact name.

NAVIGATION EFFICIENCY (critical — every round counts):
- Prefer cd with absolute paths (/root/Section/Subsection) or relative paths (Section/Sub) \
to reach target nodes in ONE command instead of multiple cd steps.
- Do NOT ls before cd if keyword hints or find results already tell you which node to enter.
- Do NOT cd into nodes one level at a time when you can use a multi-segment path.

Rules:
- Output exactly ONE command per response, nothing else.
- Content from cat is automatically saved as evidence — don't re-cat the same node.
- Do not cat or cd into nodes you have already visited.
- If the current branch has nothing relevant, use cd .. to go back.
- If you're at the root and no children seem relevant, use done.

STOPPING RULES (critical — follow these strictly):
- After cat collects evidence, immediately check: does the collected text contain information \
  that answers or relates to the user's question? If YES, output done. Do NOT continue searching.
- Do NOT run grep after cat — cat already collected the full content. grep is for locating \
  content BEFORE cat, not after.
- If ls shows '(no navigation data)' or no children, you are at a leaf node. Use cat to read it \
  or cd .. to go back. Do NOT ls again.
- When remaining rounds are low (≤2), prefer done over exploring new branches."""


def worker_navigation(params: NavigationParams) -> tuple[str, str]:
    """Build (system, user) prompt pair for a navigation round."""
    query = params.query
    breadcrumb = params.breadcrumb
    evidence_summary = params.evidence_summary
    remaining = params.remaining
    max_rounds = params.max_rounds

    task_section = (
        f"\nYour specific task: {params.task}\n(This is a sub-task for the original query.)"
        if params.task
        else ""
    )

    missing_section = (
        f"\nPotentially missing info: {params.missing_info}"
        if params.missing_info
        else ""
    )

    last_feedback_section = (
        f"\nLast command result:\n{params.last_feedback}\n"
        if params.last_feedback
        else ""
    )

    history_section = (
        ""
        if params.history == "(no history yet)"
        else f"\nPrevious rounds:\n{params.history}\n"
    )

    visited_section = (
        ""
        if params.visited_titles == "(none)"
        else f"\nAlready visited (do not re-read these): {params.visited_titles}"
    )

    plan_section = (
        ""
        if not params.plan
        else f"\nNavigation plan (follow this as guidance, adapt if needed):\n{params.plan}\n"
    )

    keyword_section = f"\n{params.keyword_hints}" if params.keyword_hints else ""

    intent_section = (
        f"\nQuery context: {params.intent_context}" if params.intent_context else ""
    )

    user = (
        f"{last_feedback_section}"
        f"User question: {query}{task_section}{intent_section}\n"
        f"\nCurrent position: /{breadcrumb}\n"
        f"Collected evidence:\n"
        f"{evidence_summary}{missing_section}{keyword_section}{visited_section}{plan_section}\n"
        f"{history_section}"
        f"Remaining rounds: {remaining}/{max_rounds}\n"
        f"\nCommand:"
    )

    return _WORKER_NAVIGATION_SYSTEM, user


# ---------------------------------------------------------------------------
# 2. Worker Dispatch (first-round prompt when Orchestrator dispatches)
# ---------------------------------------------------------------------------

@dataclass
class WorkerDispatchParams:
    original_query: str
    task: str
    doc_name: str
    breadcrumb: str


def worker_dispatch(params: WorkerDispatchParams) -> tuple[str, str]:
    """Build (system, user) prompt pair for the first round of a dispatched Worker."""
    doc_name = params.doc_name
    original_query = params.original_query
    task = params.task
    breadcrumb = params.breadcrumb

    system = (
        f'You are a document navigation assistant. You are searching inside the document '
        f'"{doc_name}" for specific information.\n'
        f"\n"
        f"Available commands: ls, cd <name> (supports Section/Sub paths and /root/Section absolute paths), "
        f"cd .., cat, cat <name>, head <name>, find <keyword>, findtree <pattern>, grep <regex>, wc <name>, "
        f"pwd, check, done\n"
        f"\n"
        f"SEARCH STRATEGY:\n"
        f"- Prefer find <keyword> to jump directly to relevant sections over manual ls→cd exploration.\n"
        f"- When find results include content snippets that answer your task, cd to that node and cat it immediately.\n"
        f"- Use multi-segment paths (e.g. cd Research Labs/Lab A) to reach targets in ONE command.\n"
        f"- Do NOT ls before cd if find results already tell you which node to enter.\n"
        f"- Use findtree when you know a section title pattern but not the exact name.\n"
        f"\n"
        f"Rules:\n"
        f"- Output exactly ONE command per response.\n"
        f"- Content from cat is automatically saved as evidence.\n"
        f"- After cat collects evidence, if it relates to your task, use done immediately.\n"
        f"- Do NOT grep after cat — cat already collected the full content.\n"
        f"- If ls shows no children, use cat to read the current node or cd .. to go back.\n"
        f"- When evidence is sufficient, use done."
    )

    user = (
        f"Original question: {original_query}\n"
        f"Your task: {task}\n"
        f"Document: {doc_name}\n"
        f"Current position: /{breadcrumb}\n"
        f"\nCommand:"
    )

    return system, user


# ---------------------------------------------------------------------------
# 3. Orchestrator Analysis (multi-doc Phase 1)
# ---------------------------------------------------------------------------

@dataclass
class OrchestratorAnalysisParams:
    query: str
    doc_cards: str
    find_results: str
    intent_context: str


def orchestrator_analysis(params: OrchestratorAnalysisParams) -> tuple[str, str]:
    """Build (system, user) prompt pair for Orchestrator document analysis."""
    system = (
        "You are a multi-document retrieval coordinator. Analyze the user's question, "
        "review the available documents, and decide which documents to search and what to look for in each.\n"
        "\n"
        "Output format — for each relevant document, output a block:\n"
        "- doc: <number>\n"
        "  reason: <why this document is relevant>\n"
        "  task: <what specific information to find in this document>\n"
        "\n"
        "Only include documents that are likely to contain relevant information.\n"
        "If the cross-document search results already fully answer the question, respond with just: ALREADY_ANSWERED"
    )

    user = (
        f"Available documents:\n"
        f"{params.doc_cards}\n"
        f"\nCross-document search results:\n"
        f"{params.find_results}\n"
        f"{params.intent_context}\n"
        f"\nUser question: {params.query}\n"
        f"\nRelevant documents:"
    )

    return system, user


# ---------------------------------------------------------------------------
# 4. Check (evidence sufficiency evaluation)
# ---------------------------------------------------------------------------

def check_sufficiency(query: str, evidence_summary: str) -> tuple[str, str]:
    """Build (system, user) prompt pair for evidence sufficiency evaluation."""
    system = (
        "You evaluate whether collected evidence contains information that can answer or "
        "relate to the user's question. The evidence is raw document text — it does not need to be "
        "a complete or perfect answer. If the evidence mentions or addresses the key concepts from "
        "the question, it is sufficient.\n"
        "\n"
        "Respond with ONLY 'SUFFICIENT' or 'INSUFFICIENT' followed by a one-line reason.\n"
        "\n"
        "Guidelines:\n"
        "- If the evidence text contains any information directly related to the question's key terms, "
        "respond SUFFICIENT.\n"
        "- If the evidence is completely unrelated or empty, respond INSUFFICIENT.\n"
        "- Default to SUFFICIENT unless the evidence is clearly irrelevant."
    )

    user = (
        f"Question: {query}\n\n"
        f"Collected evidence:\n"
        f"{evidence_summary}\n\n"
        f"Is this sufficient?"
    )

    return system, user


# ---------------------------------------------------------------------------
# 5. Navigation Planning (Phase 1.5)
# ---------------------------------------------------------------------------

def build_plan_prompt(
    query: str,
    ls_output: str,
    doc_name: str,
    keyword_hints_section: str = "",
    semantic_hints: str = "",
    intent_signals: str = "",
    task: str | None = None,
) -> tuple[str, str]:
    """Build the Phase 1.5 navigation planning prompt."""
    task_section = f"\nYour specific task: {task}" if task else ""

    system = (
        "You are a document navigation planner. Given a user question, the top-level "
        "document structure, keyword index matches, and semantic hints, output a brief navigation "
        "plan: which sections to visit and in what order. Prioritize sections that matched keywords "
        "or semantic hints. The plan should be 2-5 steps. Each step should be a specific action "
        'like "cd to X, then cat Y" or "grep for Z in current subtree". '
        "Pay attention to 'Can answer' and 'Topics' annotations in the structure listing — "
        "they indicate what questions each section addresses. "
        "Output only the plan, nothing else.\n\n"
        'Example plan for "What is the Q1 revenue?":\n'
        "1. cd to Revenue (matched keyword 'revenue')\n"
        "2. ls to see sub-sections\n"
        "3. cat Q1 Report\n"
        "4. check\n"
        "5. done"
    )

    user = (
        f"Document: {doc_name}\n"
        f"Top-level structure:\n{ls_output}{keyword_hints_section}{semantic_hints}{intent_signals}"
        f"User question: {query}{task_section}\n\n"
        f"Navigation plan:"
    )

    return system, user


# ---------------------------------------------------------------------------
# 6. Worker Re-plan (after insufficient check)
# ---------------------------------------------------------------------------

def build_replan_prompt(
    query: str,
    task: str | None,
    path_str: str,
    evidence_summary: str,
    missing_info: str,
    visited_titles: str,
    current_children: str,
    sibling_hints: str,
    remaining: int,
    max_rounds: int,
) -> tuple[str, str]:
    """Build a focused re-planning prompt when check returns INSUFFICIENT."""
    task_section = f"\nOriginal sub-task: {task}" if task else ""

    system = (
        "You are re-planning a document navigation strategy. The previous plan did not "
        "find sufficient evidence. Given what's been found and what's still missing, generate a "
        "focused 2-3 step plan. Each step should be a specific action like "
        '"cd to X, then cat Y" or "grep for Z in current subtree". '
        "Prefer exploring unvisited branches. If current branch is exhausted, cd .. and try "
        "a different path. Output only the plan, nothing else."
    )

    user = (
        f"Original question: {query}{task_section}\n"
        f"Current position: /{path_str}\n"
        f"Evidence collected so far:\n{evidence_summary}\n"
        f"What's missing: {missing_info}\n"
        f"Already visited: {visited_titles}\n"
        f"{current_children}"
        f"{sibling_hints}"
        f"Remaining rounds: {remaining}/{max_rounds}\n\n"
        f"Revised navigation plan:"
    )

    return system, user


# ---------------------------------------------------------------------------
# 7. Orchestrator Replan (after insufficient cross-doc evidence)
# ---------------------------------------------------------------------------

def orchestrator_replan_prompt(
    query: str,
    missing_info: str,
    evidence_summary: str,
    dispatched_indices: list[int],
    doc_cards: str,
    keywords_text: str = "",
) -> tuple[str, str]:
    """Build the Orchestrator re-dispatch prompt after insufficient evidence."""
    dispatched_text = (
        ", ".join(f"doc {i + 1}" for i in dispatched_indices)
        if dispatched_indices
        else "None"
    )

    system = (
        "You are a multi-document retrieval coordinator. The first round of evidence "
        "collection was insufficient to fully answer the query. Review what was collected, "
        "what's missing, and decide which additional documents to query.\n"
        "\n"
        "Output format — for each additional document to query, output a block:\n"
        "- doc: <number>\n"
        "  reason: <why this document may have the missing information>\n"
        "  task: <what specific information to find>\n"
        "\n"
        "Only include documents not yet dispatched. If no additional documents are likely to help, "
        "respond with: NO_ADDITIONAL_DOCS"
    )

    user = (
        f"Original question: {query}\n"
        f"\nMissing information: {missing_info}\n"
        f"\nCollected evidence so far:\n"
        f"{evidence_summary}\n"
        f"\nAlready dispatched documents: {dispatched_text}\n"
        f"\nAvailable documents (all):\n"
        f"{doc_cards}{keywords_text}\n"
        f"\nAdditional documents to query:"
    )

    return system, user


# ---------------------------------------------------------------------------
# Parsing utilities
# ---------------------------------------------------------------------------

@dataclass
class DispatchEntry:
    """A single dispatch entry parsed from orchestrator analysis."""

    doc_idx: int
    reason: str
    task: str


def parse_dispatch_plan(llm_output: str, total_docs: int) -> list[DispatchEntry] | None:
    """Parse the LLM output from orchestrator analysis into dispatch entries.

    Returns None if the response is "ALREADY_ANSWERED".
    Returns empty list if no valid dispatch entries found.
    """
    trimmed = llm_output.strip()

    if trimmed.startswith("ALREADY_ANSWERED"):
        return None

    entries: list[DispatchEntry] = []
    current_doc_idx: int | None = None
    current_reason = ""
    current_task = ""

    for line in trimmed.splitlines():
        line = line.strip()

        if line.startswith("- doc:"):
            # Flush previous entry
            if current_doc_idx is not None:
                entries.append(DispatchEntry(
                    doc_idx=current_doc_idx,
                    reason=current_reason,
                    task=current_task,
                ))
                current_reason = ""
                current_task = ""

            rest = line[len("- doc:"):].strip().rstrip(",")
            try:
                doc_num = int(rest)
            except ValueError:
                continue
            if 0 < doc_num <= total_docs:
                current_doc_idx = doc_num - 1  # Convert to 0-based

        elif line.startswith("reason:"):
            current_reason = line[len("reason:"):].strip()

        elif line.startswith("task:"):
            current_task = line[len("task:"):].strip()

    # Flush last entry
    if current_doc_idx is not None:
        entries.append(DispatchEntry(
            doc_idx=current_doc_idx,
            reason=current_reason,
            task=current_task,
        ))

    return entries


def parse_sufficiency_response(response: str) -> bool:
    """Parse the sufficiency check response. Returns True if SUFFICIENT."""
    upper = response.strip().upper()
    return upper.startswith("SUFFICIENT") and not upper.startswith("INSUFFICIENT")


def parse_replan_response(
    response: str,
    total_docs: int,
    dispatched: list[int],
) -> list[DispatchEntry]:
    """Parse the Orchestrator replan response into dispatch entries.

    Only includes documents not already dispatched.
    """
    trimmed = response.strip()

    if trimmed.startswith("NO_ADDITIONAL_DOCS"):
        return []

    entries: list[DispatchEntry] = []
    current_doc_idx: int | None = None
    current_reason = ""
    current_task = ""

    for line in trimmed.splitlines():
        line = line.strip()

        if line.startswith("- doc:"):
            # Flush previous
            if current_doc_idx is not None:
                entries.append(DispatchEntry(
                    doc_idx=current_doc_idx,
                    reason=current_reason,
                    task=current_task,
                ))
                current_reason = ""
                current_task = ""

            rest = line[len("- doc:"):].strip().rstrip(",")
            try:
                doc_num = int(rest)
            except ValueError:
                continue
            if 0 < doc_num <= total_docs:
                idx = doc_num - 1
                if idx not in dispatched:
                    current_doc_idx = idx

        elif line.startswith("reason:"):
            current_reason = line[len("reason:"):].strip()

        elif line.startswith("task:"):
            current_task = line[len("task:"):].strip()

    # Flush last
    if current_doc_idx is not None:
        entries.append(DispatchEntry(
            doc_idx=current_doc_idx,
            reason=current_reason,
            task=current_task,
        ))

    return entries
