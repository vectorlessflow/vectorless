"""Evidence sufficiency evaluation via LLM.

Provides structured evaluation with coverage, quality, and specific missing aspects.
"""

from __future__ import annotations

import json
import logging
from dataclasses import dataclass, field

from vectorless.ask.types import WorkerEvidence
from vectorless.llm_client import LLMClient

logger = logging.getLogger(__name__)


@dataclass
class EvalResult:
    """Structured result of evidence sufficiency evaluation."""

    sufficient: bool
    missing_info: str
    coverage: float          # 0.0-1.0, how much of the query the evidence covers
    quality_score: float     # 0.0-1.0, average relevance of evidence
    missing_aspects: list[str] = field(default_factory=list)
    relevant_evidence_ids: list[str] = field(default_factory=list)

    @property
    def needs_replan(self) -> bool:
        """Whether the orchestrator should replan and dispatch more workers."""
        return not self.sufficient and bool(self.missing_aspects)


# ---------------------------------------------------------------------------
# Prompt
# ---------------------------------------------------------------------------

_EVAL_SYSTEM = """\
You evaluate whether collected evidence can answer the user's question.

Analyze the evidence and respond with a JSON object:
{
  "sufficient": true/false,
  "coverage": 0.0-1.0,
  "quality": 0.0-1.0,
  "missing_aspects": ["aspect 1", "aspect 2"],
  "relevant_ids": ["node_id_1", "node_id_2"]
}

Guidelines:
- "sufficient": true if evidence addresses ALL key aspects of the question
- "coverage": fraction of the question's key aspects that the evidence addresses
- "quality": average relevance of the evidence items (1.0 = all directly relevant, 0.0 = all irrelevant)
- "missing_aspects": specific topics/angles the question asks about but evidence does not cover
- "relevant_ids": node_ids of evidence items that are actually relevant (not tangential)

Be strict: if any major aspect of the question is unaddressed, mark sufficient=false.
Be generous on coverage: if evidence partially addresses an aspect, count it as 0.5 coverage for that aspect.
"""


def _build_eval_prompt(query: str, evidence: list[WorkerEvidence]) -> tuple[str, str]:
    """Build (system, user) prompt for structured evaluation."""
    evidence_text = _format_evidence_for_eval(evidence)
    user = (
        f"Question: {query}\n\n"
        f"Evidence items:\n{evidence_text}\n\n"
        f"Evaluate the evidence."
    )
    return _EVAL_SYSTEM, user


def _format_evidence_for_eval(evidence: list[WorkerEvidence]) -> str:
    """Format evidence with node_id, title, and content for evaluation."""
    if not evidence:
        return "(no evidence collected)"
    return "\n\n".join(
        f"[{e.node_id}] {e.title} (from {e.source_path})\n{e.content}"
        for e in evidence
    )


# ---------------------------------------------------------------------------
# Parse
# ---------------------------------------------------------------------------

def _parse_eval_response(response: str, evidence: list[WorkerEvidence]) -> EvalResult:
    """Parse LLM JSON response into EvalResult."""
    try:
        data = json.loads(response)
    except json.JSONDecodeError:
        # Try to extract JSON from response
        import re
        match = re.search(r"\{.*\}", response, re.DOTALL)
        if match:
            data = json.loads(match.group())
        else:
            logger.warning("Failed to parse eval response as JSON, falling back to text analysis")
            return _parse_text_fallback(response, evidence)

    sufficient = bool(data.get("sufficient", False))
    coverage = float(data.get("coverage", 0.5))
    quality = float(data.get("quality", 0.5))
    missing = [str(a) for a in data.get("missing_aspects", [])]
    relevant_ids = [str(i) for i in data.get("relevant_ids", [])]

    # Clamp values
    coverage = max(0.0, min(1.0, coverage))
    quality = max(0.0, min(1.0, quality))

    missing_info = "; ".join(missing) if missing else ""

    return EvalResult(
        sufficient=sufficient,
        missing_info=missing_info,
        coverage=coverage,
        quality_score=quality,
        missing_aspects=missing,
        relevant_evidence_ids=relevant_ids,
    )


def _parse_text_fallback(response: str, evidence: list[WorkerEvidence]) -> EvalResult:
    """Fallback parsing when JSON parsing fails."""
    text = response.strip().upper()
    sufficient = text.startswith("SUFFICIENT")

    missing_info = ""
    missing_aspects = []
    if not sufficient:
        # Extract reason after INSUFFICIENT marker
        reason = response.strip()
        for prefix in ("INSUFFICIENT", "Insufficient"):
            if reason.startswith(prefix):
                reason = reason[len(prefix):]
                break
        reason = reason.lstrip("-: ")
        missing_info = reason if reason else "Evidence does not fully address the query."
        missing_aspects = [missing_info] if missing_info else []

    return EvalResult(
        sufficient=sufficient,
        missing_info=missing_info,
        coverage=0.5 if sufficient else 0.3,
        quality_score=0.5,
        missing_aspects=missing_aspects,
    )


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

async def evaluate(
    llm: LLMClient,
    query: str,
    evidence: list[WorkerEvidence],
) -> EvalResult:
    """Evaluate whether collected evidence is sufficient to answer the query.

    Returns a structured EvalResult with coverage, quality, and specific missing aspects.
    Propagates LLM errors — no fallback.
    """
    system, user = _build_eval_prompt(query, evidence)
    response = await llm.complete(system, user)
    return _parse_eval_response(response, evidence)


# ---------------------------------------------------------------------------
# Formatting helpers (used by orchestrator prompts)
# ---------------------------------------------------------------------------

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
