"""Query understanding — LLM-driven analysis of user queries.

Mirrors vectorless-core/vectorless-query/src/understand.rs.
"""

from __future__ import annotations

import json
import logging
import re

from vectorless.llm_client import LLMClient
from vectorless.query.plan import Complexity, QueryIntent, QueryPlan, SubQuery

logger = logging.getLogger(__name__)


async def understand(
    query: str,
    llm: LLMClient,
) -> QueryPlan:
    """Analyze a user query using LLM to produce a structured QueryPlan.

    Two-phase:
    1. Extract keywords locally (BM25-style, no LLM)
    2. Call LLM for intent classification, concepts, strategy, complexity

    Raises on LLM failure — no silent degradation.
    """
    keywords = _extract_keywords(query)

    system = (
        'You are a query analysis engine. Analyze the user\'s query and respond with a JSON object containing:\n'
        '\n'
        '- "intent": one of "factual", "analytical", "navigational", "summary"\n'
        '- "key_concepts": array of the main concepts/entities in the query (distinct from keywords)\n'
        '- "strategy_hint": one of "focused" (single-topic), "exploratory" (broad scan), '
        '"comparative" (cross-reference), or "summary" (aggregate)\n'
        '- "complexity": one of "simple", "moderate", "complex"\n'
        '- "rewritten": optional rewritten version of the query for better retrieval (null if not needed)\n'
        '- "sub_queries": array of sub-query strings if the query can be decomposed (empty array if not)\n'
        '\n'
        'Respond with ONLY the JSON object, no additional text.'
    )

    user = f"Query: {query}\nExtracted keywords: [{', '.join(keywords)}]"

    response = await llm.complete(system, user)

    if not response.strip():
        raise ValueError(
            "Query understanding failed: LLM returned an empty response. "
            "Check your API key, model, and endpoint configuration."
        )

    analysis = _parse_analysis(response)

    return QueryPlan(
        original=query,
        intent=_parse_intent(analysis.get("intent", "factual")),
        keywords=keywords,
        key_concepts=analysis.get("key_concepts", []),
        strategy_hint=analysis.get("strategy_hint", ""),
        complexity=_parse_complexity(analysis.get("complexity", "simple")),
        rewritten=_filter_rewritten(analysis.get("rewritten")),
        sub_queries=_parse_sub_queries(analysis.get("sub_queries", [])),
    )


def _extract_keywords(query: str) -> list[str]:
    """Extract keywords from query using stop word filtering."""
    stop_words = {
        "what", "is", "the", "a", "an", "how", "does", "do", "are",
        "in", "on", "at", "to", "for", "of", "with", "and", "or",
        "this", "that", "it", "from", "by", "was", "were", "be",
        "can", "could", "would", "should", "will", "has", "have",
        "had", "not", "but", "if", "then", "than", "so", "as",
        "there", "their", "they", "its", "about", "which", "when",
        "who", "whom", "all", "each", "every", "both", "few",
        "more", "most", "other", "some", "such", "no", "nor",
        "only", "own", "same", "too", "very", "just", "because",
    }
    words = re.findall(r"\b\w+\b", query.lower())
    return list(dict.fromkeys(w for w in words if w not in stop_words and len(w) > 2))


def _parse_analysis(response: str) -> dict:
    """Parse LLM response as JSON, handling markdown-wrapped output."""
    trimmed = response.strip()

    # Try to extract JSON from markdown code blocks
    if trimmed.startswith("```"):
        match = re.search(r"```(?:json)?\s*\n?(.*?)```", trimmed, re.DOTALL)
        if match:
            trimmed = match.group(1).strip()

    # Try to find a { ... } block
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

    # Last resort
    return json.loads(trimmed)


def _parse_intent(raw: str) -> QueryIntent:
    """Parse intent string to QueryIntent enum."""
    mapping = {
        "factual": QueryIntent.FACTUAL,
        "analytical": QueryIntent.ANALYTICAL,
        "navigational": QueryIntent.NAVIGATIONAL,
        "summary": QueryIntent.SUMMARY,
    }
    return mapping.get(raw.lower(), QueryIntent.FACTUAL)


def _parse_complexity(raw: str) -> Complexity:
    """Parse complexity string to Complexity enum."""
    mapping = {
        "simple": Complexity.SIMPLE,
        "moderate": Complexity.MODERATE,
        "complex": Complexity.COMPLEX,
    }
    return mapping.get(raw.lower(), Complexity.SIMPLE)


def _filter_rewritten(raw: str | None) -> list[str]:
    """Extract rewritten queries from LLM response."""
    if raw is None or not isinstance(raw, str) or not raw.strip():
        return []
    return [raw.strip()]


def _parse_sub_queries(raw: list) -> list[SubQuery]:
    """Parse sub_queries from LLM response."""
    if not isinstance(raw, list):
        return []
    result = []
    for item in raw:
        if isinstance(item, str) and item.strip():
            result.append(SubQuery(query=item.strip()))
        elif isinstance(item, dict):
            result.append(SubQuery(
                query=item.get("query", ""),
                intent=_parse_intent(item.get("intent", "factual")),
                target_docs=item.get("target_docs"),
            ))
    return result
