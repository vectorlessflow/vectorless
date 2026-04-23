"""Evidence sufficiency evaluation via LLM."""

from __future__ import annotations

from dataclasses import dataclass

from vectorless._types import WorkerEvidence
from vectorless.llm_client import LLMClient
from vectorless.prompts.agent import check_sufficiency, parse_sufficiency_response


@dataclass
class EvalResult:
    """Result of evidence sufficiency evaluation."""

    sufficient: bool
    missing_info: str


def _format_evidence_summary(evidence: list[WorkerEvidence]) -> str:
    """Format evidence with actual content for sufficiency check."""
    if not evidence:
        return "(no evidence)"
    return "\n\n".join(
        f"[{e.title}] (from {e.source_path})\n{e.content}"
        for e in evidence
    )


async def evaluate(
    llm: LLMClient,
    query: str,
    evidence: list[WorkerEvidence],
) -> EvalResult:
    """Evaluate whether collected evidence is sufficient to answer the query.

    Uses LLM to assess sufficiency. Propagates LLM errors — no fallback.
    """
    evidence_summary = _format_evidence_summary(evidence)
    system, user = check_sufficiency(query, evidence_summary)

    response = await llm.complete(system, user)
    sufficient = parse_sufficiency_response(response)

    missing_info = ""
    if not sufficient:
        reason = response.strip()
        for prefix in ("INSUFFICIENT", "Insufficient"):
            if reason.startswith(prefix):
                reason = reason[len(prefix):]
                break
        reason = reason.lstrip("-: ")
        missing_info = reason if reason else "Evidence does not fully address the query."

    return EvalResult(sufficient=sufficient, missing_info=missing_info)


def format_evidence_brief(evidence: list[WorkerEvidence]) -> str:
    """Format evidence as a brief summary (title + char count) for prompts."""
    if not evidence:
        return "(none)"
    return "\n".join(
        f"- [{e.title}] {len(e.content)} chars" for e in evidence
    )


def format_evidence_for_check(evidence: list[WorkerEvidence]) -> str:
    """Format evidence with full content for sufficiency check."""
    if not evidence:
        return "(no evidence collected yet)"
    return "\n\n".join(
        f"[{e.title}]\n{e.content}" for e in evidence
    )
