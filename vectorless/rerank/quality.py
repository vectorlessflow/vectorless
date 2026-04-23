"""LLM-based evidence quality filtering.

Evaluates whether each evidence item actually answers the question.
Low-quality or irrelevant evidence is filtered out before synthesis.
"""

from __future__ import annotations

import json
import logging
from typing import Optional

from vectorless._types import WorkerEvidence
from vectorless.llm_client import LLMClient
from vectorless.query.plan import QueryIntent

logger = logging.getLogger(__name__)

# Minimum evidence count after quality filter — keep at least this many
MIN_KEEP_COUNT = 1


async def filter_by_quality(
    evidence: list[WorkerEvidence],
    query: str,
    intent: QueryIntent,
    llm: LLMClient,
    *,
    confidence: float = 0.0,
    min_relevance: float = 0.4,
) -> list[WorkerEvidence]:
    """Filter evidence by LLM-judged relevance to the query.

    For small evidence sets (≤3), skip LLM evaluation — all evidence is kept.
    For larger sets, batch-evaluate relevance and filter below threshold.

    Args:
        evidence: Evidence items from workers.
        query: The original question.
        intent: Classified query intent.
        llm: LLM client for quality evaluation.
        confidence: Orchestrator confidence (used to adjust threshold).
        min_relevance: Minimum relevance score (0.0-1.0) to keep.

    Returns:
        Filtered evidence list.
    """
    if not evidence:
        return []

    # Skip quality filter for small evidence sets — trust the worker
    if len(evidence) <= 3:
        return evidence

    system = _quality_system_prompt(intent)
    user = _quality_user_prompt(query, evidence)

    try:
        result = await llm.complete_json(system, user, temperature=0.0)
    except Exception as e:
        logger.warning("Quality filter LLM call failed, keeping all evidence: %s", e)
        return evidence

    scores = _parse_scores(result, len(evidence))
    if not scores:
        return evidence

    # Filter by threshold
    kept = [
        ev for i, ev in enumerate(evidence)
        if scores.get(i, 0.5) >= min_relevance
    ]

    # Ensure we keep at least MIN_KEEP_COUNT
    if not kept and evidence:
        # Keep the highest-scoring evidence
        best_idx = max(scores, key=scores.get) if scores else 0
        kept = [evidence[best_idx]]

    logger.info(
        "Quality filter: %d/%d evidence kept (threshold=%.2f)",
        len(kept), len(evidence), min_relevance,
    )

    return kept


# ---------------------------------------------------------------------------
# Prompt construction
# ---------------------------------------------------------------------------


def _quality_system_prompt(intent: QueryIntent) -> str:
    intent_desc = {
        QueryIntent.FACTUAL: "a factual question seeking specific information",
        QueryIntent.ANALYTICAL: "an analytical question requiring reasoning across multiple sections",
        QueryIntent.COMPARATIVE: "a comparative question requiring contrast between items",
        QueryIntent.PROCEDURAL: "a procedural question about how to do something",
        QueryIntent.EXPLORATORY: "an exploratory question seeking overview or context",
    }.get(intent, "a question")

    return f"""You are an evidence quality evaluator. The user asked {intent_desc}.

For each evidence item, rate its relevance to the question on a scale of 0.0 to 1.0:
- 1.0: Directly and completely answers the question
- 0.8: Contains relevant information that partially answers the question
- 0.6: Tangentially related but does not directly address the question
- 0.4: Mentions related concepts but is not useful for answering
- 0.2: Barely related or only provides background context
- 0.0: Completely irrelevant

Respond with a JSON object: {{"scores": [0.8, 0.3, ...]}}
One score per evidence item, in order. Scores array length must match evidence count."""


def _quality_user_prompt(query: str, evidence: list[WorkerEvidence]) -> str:
    items = []
    for i, ev in enumerate(evidence):
        # Truncate long evidence for the prompt
        content = ev.content
        if len(content) > 300:
            content = content[:300] + "..."
        items.append(f"[{i}] {ev.title}\n{content}")

    return f"""Question: {query}

Evidence items:
{chr(10).join(items)}

Rate the relevance of each evidence item to the question."""


# ---------------------------------------------------------------------------
# Response parsing
# ---------------------------------------------------------------------------


def _parse_scores(response: dict, expected_count: int) -> dict[int, float]:
    """Parse relevance scores from LLM response."""
    scores_raw = response.get("scores", [])
    if not isinstance(scores_raw, list):
        return {}

    scores = {}
    for i, s in enumerate(scores_raw):
        if i >= expected_count:
            break
        try:
            scores[i] = float(s)
        except (ValueError, TypeError):
            scores[i] = 0.5  # default mid-score on parse failure

    return scores
