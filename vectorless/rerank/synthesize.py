"""Evidence deduplication and answer formatting.

Dedup is pure compute (no LLM). Answer formatting is intent-aware.

The Rust rerank principle: "Find what you find, return what you find."
No LLM synthesis — the evidence IS the answer.
"""

from __future__ import annotations

from dataclasses import dataclass

from vectorless.ask.types import Evidence
from vectorless.ask.reasoning.types import QueryIntent


# ---------------------------------------------------------------------------
# Deduplication (compute — mirrors vectorless-rerank/src/dedup.rs)
# ---------------------------------------------------------------------------

MIN_EVIDENCE_CHARS = 50
SIMILARITY_THRESHOLD = 0.8


def dedup(evidence: list[Evidence]) -> list[Evidence]:
    """Deduplicate evidence in three stages:
    1. Quality filter — remove evidence with < 50 chars
    2. Source dedup — keep first per unique (doc_name, source_path)
    3. Content similarity — remove near-duplicates via Jaccard similarity
    """
    # Stage 1: quality filter
    filtered = [e for e in evidence if len(e.content.strip()) >= MIN_EVIDENCE_CHARS]

    # Stage 2: source dedup — mirrors Rust dedup key = "doc_name:source_path"
    seen_keys: set[str] = set()
    stage2: list[Evidence] = []
    for e in filtered:
        doc_key = e.doc_name or "_unknown"
        key = f"{doc_key}:{e.source_path}"
        if key not in seen_keys:
            seen_keys.add(key)
            stage2.append(e)

    # Stage 3: content similarity (Jaccard)
    result: list[Evidence] = []
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
# Formatting (intent-aware — mirrors rerank/src/lib.rs)
# ---------------------------------------------------------------------------

def format_answer(
    evidence: list[Evidence],
    intent: QueryIntent = QueryIntent.FACTUAL,
) -> str:
    """Format evidence into an answer string based on query intent.

    Mirrors Rust rerank process() — no LLM, just formatting.
    """
    if not evidence:
        return ""

    if intent == QueryIntent.NAVIGATIONAL:
        return _format_locations(evidence)

    return _format_evidence_as_answer(evidence)


def _format_evidence_as_answer(evidence: list[Evidence]) -> str:
    """Format collected evidence directly as the answer.

    Mirrors Rust format_evidence_as_answer — includes doc_name attribution.
    """
    parts: list[str] = []

    for e in evidence:
        doc = e.doc_name or ""
        if doc:
            parts.append(f"[{e.node_title} — {doc}]\n{e.content}")
        else:
            parts.append(f"[{e.node_title}]\n{e.content}")

    return "\n\n".join(parts)


def _format_locations(evidence: list[Evidence]) -> str:
    """Format evidence as location references (for navigational queries).

    Mirrors Rust format_locations.
    """
    if not evidence:
        return "No matching locations found."
    result = "Found at:\n"
    for e in evidence:
        doc = e.doc_name or "unknown"
        result += f"- **{e.node_title}** in {doc} at {e.source_path}\n"
    return result


# ---------------------------------------------------------------------------
# Rerank output — mirrors rerank::types::RerankOutput
# ---------------------------------------------------------------------------

@dataclass
class RerankOutput:
    """Output from the rerank pipeline.

    Mirrors Rust RerankOutput: answer + llm_calls + confidence.
    Evidence is also included for the Python Orchestrator to assemble Output.
    """

    answer: str
    evidence: list[Evidence]
    confidence: float
    llm_calls: int = 0  # Always 0 for rerank — no LLM calls


def process(
    evidence: list[Evidence],
    intent: QueryIntent = QueryIntent.FACTUAL,
    confidence: float = 0.0,
) -> RerankOutput:
    """Run the rerank pipeline: dedup → format.

    No LLM calls — pure compute and formatting.
    Mirrors Rust rerank::process().
    """
    deduped = dedup(evidence)

    if not deduped:
        return RerankOutput(answer="", evidence=[], confidence=0.0, llm_calls=0)

    answer = format_answer(deduped, intent)

    return RerankOutput(
        answer=answer,
        evidence=deduped,
        confidence=confidence,
        llm_calls=0,
    )
