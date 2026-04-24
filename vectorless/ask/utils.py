"""Shared utilities for the ask pipeline.

Single source of truth for keyword extraction, evidence formatting,
and JSON parsing. All modules should import from here instead of
defining their own copies.
"""

from __future__ import annotations

import json
import re

from vectorless.ask.types import Evidence


# ---------------------------------------------------------------------------
# Keyword extraction — unified stop word list
# ---------------------------------------------------------------------------

_STOP_WORDS = frozenset({
    "what", "is", "the", "a", "an", "how", "does", "do", "are",
    "in", "on", "at", "to", "for", "of", "with", "and", "or",
    "this", "that", "it", "from", "by", "was", "were", "be",
    "can", "could", "would", "should", "will", "has", "have",
    "had", "not", "but", "if", "then", "than", "so", "as",
    "there", "their", "they", "its", "about", "which", "when",
    "who", "whom", "all", "each", "every", "both", "few",
    "more", "most", "other", "some", "such", "no", "nor",
    "only", "own", "same", "too", "very", "just", "because",
})


def extract_keywords(query: str) -> list[str]:
    """Extract keywords from a query using stop word filtering.

    Returns deduplicated keywords in order of first appearance.
    """
    words = re.findall(r"\b\w+\b", query.lower())
    return list(dict.fromkeys(w for w in words if w not in _STOP_WORDS and len(w) > 2))


# ---------------------------------------------------------------------------
# Evidence formatting — single source of truth
# ---------------------------------------------------------------------------

def format_evidence(evidence: list[Evidence]) -> str:
    """Format evidence with source attribution.

    Used by orchestrator, verifier, and evaluation modules.
    """
    if not evidence:
        return "(no evidence)"
    return "\n\n".join(
        f"[{e.node_title}] (from {e.doc_name or 'unknown'})\n{e.content}"
        for e in evidence
    )


# ---------------------------------------------------------------------------
# JSON parsing — single source of truth
# ---------------------------------------------------------------------------

def parse_json_response(response: str) -> dict:
    """Parse LLM response as JSON, handling markdown-wrapped output.

    Raises ``ValueError`` if the response cannot be parsed as JSON.
    """
    trimmed = response.strip()

    if trimmed.startswith("```"):
        match = re.search(r"```(?:json)?\s*\n?(.*?)```", trimmed, re.DOTALL)
        if match:
            trimmed = match.group(1).strip()

    start = trimmed.find("{")
    if start != -1:
        depth = 0
        for i in range(start, len(trimmed)):
            if trimmed[i] == "{":
                depth += 1
            elif trimmed[i] == "}":
                depth -= 1
                if depth == 0:
                    candidate = trimmed[start : i + 1]
                    try:
                        return json.loads(candidate)
                    except json.JSONDecodeError:
                        break

    try:
        return json.loads(trimmed)
    except json.JSONDecodeError as e:
        raise ValueError(f"Failed to parse LLM response as JSON: {e}") from e
