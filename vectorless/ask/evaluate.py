"""Evaluate cross-document evidence sufficiency via LLM.

Mirrors vectorless-agent/src/orchestrator/evaluate.rs.

Two evaluation modes:
1. Cross-doc evaluation (Orchestrator level) — simple SUFFICIENT/INSUFFICIENT text parse
2. Structured evaluation (Worker level) — JSON with coverage/quality/missing_aspects

Both use the same Evidence type from types.py.
"""

from __future__ import annotations

import json
import logging
import re

from vectorless.ask.types import EvalResult, Evidence
from vectorless.llm_client import LLMClient

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Cross-doc evaluation prompt — mirrors Rust orchestrator/evaluate.rs
# ---------------------------------------------------------------------------

def _build_cross_eval_prompt(query: str, evidence: list[Evidence]) -> tuple[str, str]:
    """Build (system, user) prompt for cross-document sufficiency evaluation.

    Uses the same SUFFICIENT/INSUFFICIENT text format as Rust — simple and robust.
    """
    evidence_summary = _format_evidence_summary(evidence)
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


def _format_evidence_summary(evidence: list[Evidence]) -> str:
    """Format evidence with source attribution for evaluation.

    Mirrors Rust format_evidence_summary — includes doc_name for cross-doc context.
    """
    if not evidence:
        return "(no evidence)"
    return "\n\n".join(
        f"[{e.node_title}] (from {e.doc_name or 'unknown'})\n{e.content}"
        for e in evidence
    )


# ---------------------------------------------------------------------------
# Parse cross-doc evaluation response — mirrors Rust parse_sufficiency_response
# ---------------------------------------------------------------------------

def _parse_sufficiency_response(response: str) -> bool:
    """Parse the sufficiency check response. Returns True if SUFFICIENT."""
    upper = response.strip().upper()
    return upper.startswith("SUFFICIENT") and not upper.startswith("INSUFFICIENT")


def _extract_missing_info(response: str) -> str:
    """Extract missing info description from an INSUFFICIENT response."""
    reason = response.strip()
    for prefix in ("INSUFFICIENT", "Insufficient"):
        if reason.startswith(prefix):
            reason = reason[len(prefix):]
            break
    reason = reason.lstrip("-: ")
    return reason if reason else "Evidence does not fully address the query."


# ---------------------------------------------------------------------------
# Structured evaluation prompt (for detailed analysis)
# ---------------------------------------------------------------------------

_EVAL_SYSTEM = """\
You evaluate whether collected evidence can answer the user's question.

Analyze the evidence and respond with a JSON object:
{
  "sufficient": true/false,
  "coverage": 0.0-1.0,
  "quality": 0.0-1.0,
  "missing_aspects": ["aspect 1", "aspect 2"],
  "relevant_ids": ["node_title_1", "node_title_2"]
}

Guidelines:
- "sufficient": true if evidence addresses ALL key aspects of the question
- "coverage": fraction of the question's key aspects that the evidence addresses
- "quality": average relevance of the evidence items (1.0 = all directly relevant, 0.0 = all irrelevant)
- "missing_aspects": specific topics/angles the question asks about but evidence does not cover
- "relevant_ids": node_titles of evidence items that are actually relevant (not tangential)

Be strict: if any major aspect of the question is unaddressed, mark sufficient=false.
Be generous on coverage: if evidence partially addresses an aspect, count it as 0.5 coverage for that aspect.
"""


def _build_structured_eval_prompt(query: str, evidence: list[Evidence]) -> tuple[str, str]:
    """Build (system, user) prompt for structured evaluation."""
    evidence_text = _format_evidence_summary(evidence)
    user = (
        f"Question: {query}\n\n"
        f"Evidence items:\n{evidence_text}\n\n"
        f"Evaluate the evidence."
    )
    return _EVAL_SYSTEM, user


def _parse_structured_response(response: str) -> EvalResult:
    """Parse LLM JSON response into EvalResult."""
    try:
        data = json.loads(response)
    except json.JSONDecodeError:
        match = re.search(r"\{.*\}", response, re.DOTALL)
        if match:
            data = json.loads(match.group())
        else:
            logger.warning("Failed to parse eval response as JSON, falling back to text")
            return _parse_text_fallback(response)

    sufficient = bool(data.get("sufficient", False))
    coverage = float(data.get("coverage", 0.5))
    quality = float(data.get("quality", 0.5))
    missing = [str(a) for a in data.get("missing_aspects", [])]
    relevant_ids = [str(i) for i in data.get("relevant_ids", [])]

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


def _parse_text_fallback(response: str) -> EvalResult:
    """Fallback parsing when JSON parsing fails."""
    text = response.strip().upper()
    sufficient = text.startswith("SUFFICIENT")

    missing_info = ""
    missing_aspects: list[str] = []
    if not sufficient:
        missing_info = _extract_missing_info(response)
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
    evidence: list[Evidence],
    *,
    structured: bool = False,
) -> EvalResult:
    """Evaluate whether collected evidence is sufficient to answer the query.

    Two modes:
    - structured=False (default): cross-doc SUFFICIENT/INSUFFICIENT — mirrors Rust
    - structured=True: JSON with coverage/quality scores

    Propagates LLM errors — no fallback.
    """
    if structured:
        system, user = _build_structured_eval_prompt(query, evidence)
        response = await llm.complete(system, user)
        return _parse_structured_response(response)

    # Cross-doc evaluation (mirrors Rust orchestrator/evaluate.rs)
    system, user = _build_cross_eval_prompt(query, evidence)
    response = await llm.complete(system, user)

    sufficient = _parse_sufficiency_response(response)
    missing_info = "" if sufficient else _extract_missing_info(response)

    return EvalResult(
        sufficient=sufficient,
        missing_info=missing_info,
        coverage=0.7 if sufficient else 0.3,
        quality_score=0.7 if sufficient else 0.3,
        missing_aspects=[missing_info] if missing_info else [],
    )
