"""QueryAnalyzer — single-call query reasoning.

One structured LLM call classifies the query (intent, complexity), extracts key
concepts and salient entities, and proposes alternate phrasings for matching.
Keywords are derived locally (no LLM). This replaces the former 3-stage pipeline
(classify → deep analysis → strategy, up to 3 sequential calls).
"""

from __future__ import annotations

import logging

from pydantic import BaseModel, Field

from vectorless.llm_client import LLMClient
from vectorless.ask.utils import extract_keywords
from vectorless.ask.reasoning.types import (
    Complexity,
    EntityRef,
    QueryAnalysis,
    QueryIntent,
    RetrievalStrategy,
)

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Structured output (instructor / pydantic)
# ---------------------------------------------------------------------------

class _QueryAnalysisLLM(BaseModel):
    """The analyzer's structured view of a user query."""

    intent: str = Field(
        default="factual",
        description="One of: factual, analytical, navigational, summary, comparative, procedural.",
    )
    complexity: str = Field(
        default="simple",
        description="One of: simple, moderate, complex.",
    )
    key_concepts: list[str] = Field(
        default_factory=list,
        description="The core concepts the answer must cover.",
    )
    entities: list[str] = Field(
        default_factory=list,
        description="Salient named entities mentioned in the question (people, orgs, products, terms).",
    )
    rewritten: list[str] = Field(
        default_factory=list,
        description="0-3 alternate phrasings of the question, useful for matching section titles.",
    )
    strategy: str = Field(
        default="focused",
        description="Retrieval shape: focused, exploratory, comparative, or summary.",
    )


# ---------------------------------------------------------------------------
# Analyzer
# ---------------------------------------------------------------------------

class QueryAnalyzer:
    """Single-call query reasoning. ``analyze()`` returns a QueryAnalysis."""

    async def analyze(self, query: str, llm: LLMClient) -> QueryAnalysis:
        """Analyze a user query with one structured LLM call.

        Raises (via the LLM client) on hard LLM failure — no silent degradation.
        """
        keywords = extract_keywords(query)

        system = (
            "You analyze a user's question to guide document retrieval over a tree of "
            "sections. Classify its intent and complexity, extract the key concepts and "
            "salient entities, and suggest a few alternate phrasings that would help match "
            "section titles. Be concise."
        )
        user = f"Question: {query}\nKeywords: {', '.join(keywords) or '(none)'}"

        m = await llm.complete_structured(system, user, _QueryAnalysisLLM)

        return QueryAnalysis(
            original=query,
            rewritten=[r.strip() for r in m.rewritten if r and r.strip()],
            intent=_parse_intent(m.intent),
            complexity=_parse_complexity(m.complexity),
            keywords=keywords,
            key_concepts=[c for c in m.key_concepts if c],
            entities=[EntityRef(name=n, entity_type="concept") for n in m.entities if n],
            strategy=RetrievalStrategy(strategy_type=_map_strategy(m.strategy)),
        )


# ---------------------------------------------------------------------------
# Parsing helpers
# ---------------------------------------------------------------------------

def _parse_intent(raw: str) -> QueryIntent:
    return {
        "factual": QueryIntent.FACTUAL,
        "analytical": QueryIntent.ANALYTICAL,
        "navigational": QueryIntent.NAVIGATIONAL,
        "summary": QueryIntent.SUMMARY,
        "comparative": QueryIntent.COMPARATIVE,
        "procedural": QueryIntent.PROCEDURAL,
    }.get((raw or "").lower(), QueryIntent.FACTUAL)


def _parse_complexity(raw: str) -> Complexity:
    return {
        "simple": Complexity.SIMPLE,
        "moderate": Complexity.MODERATE,
        "complex": Complexity.COMPLEX,
    }.get((raw or "").lower(), Complexity.SIMPLE)


def _map_strategy(hint: str) -> str:
    return {
        "focused": "focused",
        "exploratory": "exploratory",
        "comparative": "comparative",
        "summary": "summary",
    }.get((hint or "").lower(), "focused")
