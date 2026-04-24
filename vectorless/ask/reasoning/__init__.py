"""Query reasoning module — multi-stage query analysis pipeline."""

from vectorless.ask.reasoning.types import (
    Ambiguity,
    AmbiguityType,
    Complexity,
    EntityRef,
    QueryAnalysis,
    QueryIntent,
    RetrievalStrategy,
    SubQuery,
    TemporalConstraint,
)
from vectorless.ask.reasoning.analyzer import QueryAnalyzer

__all__ = [
    "Ambiguity",
    "AmbiguityType",
    "Complexity",
    "EntityRef",
    "QueryAnalysis",
    "QueryAnalyzer",
    "QueryIntent",
    "RetrievalStrategy",
    "SubQuery",
    "TemporalConstraint",
]
