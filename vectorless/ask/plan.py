"""Query plan types — mirrors vectorless-query types."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from vectorless.ask.reasoning.types import QueryAnalysis


class QueryIntent(str, Enum):
    """Detected intent of a user query."""
    FACTUAL = "factual"
    ANALYTICAL = "analytical"
    NAVIGATIONAL = "navigational"
    SUMMARY = "summary"


class Complexity(str, Enum):
    """Estimated query complexity."""
    SIMPLE = "simple"
    MODERATE = "moderate"
    COMPLEX = "complex"


@dataclass
class SubQuery:
    """A decomposed sub-query from a complex query."""
    query: str
    intent: QueryIntent = QueryIntent.FACTUAL
    target_docs: list[str] | None = None


@dataclass
class QueryPlan:
    """Structured analysis of a user query.

    Produced by query understanding, consumed by Orchestrator and Workers.
    """
    original: str
    intent: QueryIntent = QueryIntent.FACTUAL
    keywords: list[str] = field(default_factory=list)
    key_concepts: list[str] = field(default_factory=list)
    strategy_hint: str = ""
    complexity: Complexity = Complexity.SIMPLE
    rewritten: list[str] = field(default_factory=list)
    sub_queries: list[SubQuery] = field(default_factory=list)

    def intent_context(self) -> str:
        """Format intent context string for prompts."""
        parts = [f"Query intent: {self.intent.value} (complexity: {self.complexity.value})"]
        if self.key_concepts:
            parts.append(f"Key concepts: {', '.join(self.key_concepts)}")
        if self.strategy_hint:
            parts.append(f"Retrieval strategy: {self.strategy_hint}")
        if self.rewritten:
            parts.append(f"Rewritten queries for matching: {'; '.join(self.rewritten)}")
        return "\n" + "\n".join(parts)

    def to_query_analysis(self) -> QueryAnalysis:
        """Convert this QueryPlan to a QueryAnalysis for backward compatibility."""
        from vectorless.ask.reasoning.types import (
            QueryAnalysis as QA,
            RetrievalStrategy,
        )
        return QA(
            original=self.original,
            rewritten=self.rewritten,
            intent=QueryIntent(self.intent.value),
            complexity=Complexity(self.complexity.value),
            keywords=self.keywords,
            key_concepts=self.key_concepts,
            strategy=RetrievalStrategy(strategy_type=self.strategy_hint or "focused"),
            sub_queries=[
                SubQuery(query=sq.query, target_docs=sq.target_docs)
                for sq in self.sub_queries
            ],
        )
