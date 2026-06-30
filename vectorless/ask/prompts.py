"""Prompt templates and parsing for the Orchestrator's document-selection step.

The per-document reasoning lives in the Scout (see ``scout.py``); the only
Orchestrator-level prompt is the analysis that picks which documents to search
and what each should look for.
"""

from __future__ import annotations

from dataclasses import dataclass

from vectorless.ask.types import DispatchEntry


# ---------------------------------------------------------------------------
# Orchestrator analysis (document selection)
# ---------------------------------------------------------------------------

@dataclass
class OrchestratorAnalysisParams:
    query: str
    doc_cards: str
    find_results: str
    intent_context: str


def orchestrator_analysis(params: OrchestratorAnalysisParams) -> tuple[str, str]:
    """Build (system, user) prompt pair for Orchestrator document selection."""
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
# Parsing
# ---------------------------------------------------------------------------

def parse_dispatch_plan(llm_output: str, total_docs: int) -> list[DispatchEntry] | None:
    """Parse the orchestrator analysis output into dispatch entries.

    Returns None if the response is "ALREADY_ANSWERED"; an empty list if no
    valid dispatch entries are found.
    """
    trimmed = llm_output.strip()

    if trimmed.startswith("ALREADY_ANSWERED"):
        return None

    entries: list[DispatchEntry] = []
    current_doc_idx: int | None = None
    current_reason = ""
    current_task = ""

    for raw in trimmed.splitlines():
        line = raw.strip()

        if line.startswith("- doc:"):
            if current_doc_idx is not None:
                entries.append(DispatchEntry(
                    doc_idx=current_doc_idx, reason=current_reason, task=current_task,
                ))
                current_reason = ""
                current_task = ""

            rest = line[len("- doc:"):].strip().rstrip(",")
            try:
                doc_num = int(rest)
            except ValueError:
                continue
            if 0 < doc_num <= total_docs:
                current_doc_idx = doc_num - 1  # 0-based

        elif line.startswith("reason:"):
            current_reason = line[len("reason:"):].strip()

        elif line.startswith("task:"):
            current_task = line[len("task:"):].strip()

    if current_doc_idx is not None:
        entries.append(DispatchEntry(
            doc_idx=current_doc_idx, reason=current_reason, task=current_task,
        ))

    return entries
