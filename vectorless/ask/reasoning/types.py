"""Query reasoning types — rich analysis replacing QueryPlan.

Unlike QueryPlan which captures a shallow snapshot, QueryAnalysis is a living
object that can be re-invoked during retrieval with additional context from
verification gaps and evidence summaries.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum


class QueryIntent(str, Enum):
    """Detected intent of a user query."""
    FACTUAL = "factual"
    ANALYTICAL = "analytical"
    NAVIGATIONAL = "navigational"
    SUMMARY = "summary"
    COMPARATIVE = "comparative"
    PROCEDURAL = "procedural"


class Complexity(str, Enum):
    """Estimated query complexity."""
    SIMPLE = "simple"
    MODERATE = "moderate"
    COMPLEX = "complex"


class AmbiguityType(str, Enum):
    """Type of ambiguity detected in a query."""
    LEXICAL = "lexical"
    SCOPE = "scope"
    REFERENCE = "reference"
    TEMPORAL = "temporal"


@dataclass
class Ambiguity:
    """An ambiguity detected in the query."""
    ambiguity_type: AmbiguityType
    description: str
    possible_interpretations: list[str]
    resolution_query: str


@dataclass
class TemporalConstraint:
    """A temporal constraint extracted from the query."""
    raw: str
    resolved: str | None = None
    is_relative: bool = False


@dataclass
class EntityRef:
    """A named entity referenced in the query."""
    name: str
    entity_type: str          # "person", "org", "product", "concept"
    aliases: list[str] = field(default_factory=list)
    definition_hint: str = ""


@dataclass
class RetrievalStrategy:
    """Strategy for how to retrieve information."""
    strategy_type: str = "focused"  # "focused", "exploratory", "comparative", "summary"
    sub_strategies: list[str] = field(default_factory=list)
    target_sections: list[str] = field(default_factory=list)
    requires_cross_doc: bool = False
    estimated_depth: str = "medium"  # "shallow", "medium", "deep"


@dataclass
class SubQuery:
    """A decomposed sub-query from a complex query."""
    query: str
    intent: QueryIntent = QueryIntent.FACTUAL
    target_docs: list[str] | None = None


@dataclass
class QueryAnalysis:
    """Rich analysis of a user query.

    Produced by QueryAnalyzer, consumed by Orchestrator and Workers.
    Can be re-invoked during retrieval with additional context.
    """
    original: str
    rewritten: list[str] = field(default_factory=list)
    intent: QueryIntent = QueryIntent.FACTUAL
    complexity: Complexity = Complexity.SIMPLE
    keywords: list[str] = field(default_factory=list)
    key_concepts: list[str] = field(default_factory=list)
    entities: list[EntityRef] = field(default_factory=list)
    ambiguities: list[Ambiguity] = field(default_factory=list)
    temporal_constraints: list[TemporalConstraint] = field(default_factory=list)
    sub_queries: list[SubQuery] = field(default_factory=list)
    strategy: RetrievalStrategy = field(default_factory=RetrievalStrategy)
    iteration: int = 0
    additional_context: str = ""
    previous_evidence_summary: str = ""

    def intent_context(self) -> str:
        """Format intent context string for prompts.

        Backward-compatible with QueryPlan.intent_context().
        """
        parts = [f"Query intent: {self.intent.value} (complexity: {self.complexity.value})"]
        if self.key_concepts:
            parts.append(f"Key concepts: {', '.join(self.key_concepts)}")
        if self.strategy.strategy_type:
            parts.append(f"Retrieval strategy: {self.strategy.strategy_type}")
        if self.rewritten:
            parts.append(f"Rewritten queries for matching: {'; '.join(self.rewritten)}")
        if self.entities:
            entity_names = ", ".join(e.name for e in self.entities[:5])
            parts.append(f"Key entities: {entity_names}")
        if self.additional_context:
            parts.append(f"Additional context: {self.additional_context}")
        return "\n" + "\n".join(parts)
