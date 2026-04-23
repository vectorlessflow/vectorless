"""Evidence deduplication and answer formatting.

Mirrors vectorless-core/vectorless-rerank/src/.
Dedup is pure compute (no LLM). Answer formatting is intent-aware.

The Rust rerank principle: "Find what you find, return what you find."
No LLM synthesis — the evidence IS the answer.
"""

from __future__ import annotations

from dataclasses import dataclass

from vectorless._types import WorkerEvidence
from vectorless.query.plan import QueryIntent


# ---------------------------------------------------------------------------
# Deduplication (compute — mirrors vectorless-rerank/src/dedup.rs)
# ---------------------------------------------------------------------------

MIN_EVIDENCE_CHARS = 50
SIMILARITY_THRESHOLD = 0.8


def dedup(evidence: list[WorkerEvidence]) -> list[WorkerEvidence]:
    """Deduplicate evidence in three stages:
    1. Quality filter — remove evidence with < 50 chars
    2. Source dedup — keep first per unique (source_path, node_title)
    3. Content similarity — remove near-duplicates via Jaccard similarity
    """
    # Stage 1: quality filter
    filtered = [e for e in evidence if len(e.content.strip()) >= MIN_EVIDENCE_CHARS]

    # Stage 2: source dedup
    seen_keys: set[str] = set()
    stage2: list[WorkerEvidence] = []
    for e in filtered:
        key = f"{e.source_path}::{e.title}"
        if key not in seen_keys:
            seen_keys.add(key)
            stage2.append(e)

    # Stage 3: content similarity (Jaccard)
    result: list[WorkerEvidence] = []
    for e in stage2:
        is_duplicate = False
        for existing in result:
            if _jaccard_similarity(e.content, existing.content) >= SIMILARITY_THRESHOLD:
                is_duplicate = True
                break
        if not is_duplicate:
            result.append(e)

    return result


def _jaccard_similarity(a: str, b: str) -> float:
    """Compute Jaccard similarity between two strings (word-level)."""
    words_a = set(a.lower().split())
    words_b = set(b.lower().split())
    if not words_a and not words_b:
        return 1.0
    if not words_a or not words_b:
        return 0.0
    intersection = words_a & words_b
    union = words_a | words_b
    return len(intersection) / len(union)


# ---------------------------------------------------------------------------
# Formatting (intent-aware)
# ---------------------------------------------------------------------------

def format_answer(
    evidence: list[WorkerEvidence],
    intent: QueryIntent = QueryIntent.FACTUAL,
) -> str:
    """Format evidence into an answer string based on query intent.

    Mirrors vectorless-rerank process() — no LLM, just formatting.
    """
    if not evidence:
        return ""

    if intent == QueryIntent.NAVIGATIONAL:
        return _format_locations(evidence)

    return _format_evidence_as_answer(evidence)


def _format_evidence_as_answer(evidence: list[WorkerEvidence]) -> str:
    """Format evidence as a structured answer."""
    parts: list[str] = []

    for i, e in enumerate(evidence, 1):
        source = e.source_path
        parts.append(f"[{i}] {source}")
        parts.append(e.content)
        parts.append("")

    return "\n".join(parts).strip()


def _format_locations(evidence: list[WorkerEvidence]) -> str:
    """Format evidence as location references (for navigational queries)."""
    lines: list[str] = []
    for e in evidence:
        lines.append(f"- {e.source_path}: {e.title}")
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Rerank output
# ---------------------------------------------------------------------------

@dataclass
class RerankOutput:
    """Output from the rerank pipeline."""
    answer: str
    evidence: list[WorkerEvidence]
    confidence: float


def process(
    evidence: list[WorkerEvidence],
    intent: QueryIntent = QueryIntent.FACTUAL,
    confidence: float = 0.0,
) -> RerankOutput:
    """Run the rerank pipeline: dedup → format.

    No LLM calls — pure compute and formatting.
    """
    deduped = dedup(evidence)
    answer = format_answer(deduped, intent)

    return RerankOutput(
        answer=answer,
        evidence=deduped,
        confidence=confidence,
    )
