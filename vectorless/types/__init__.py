"""Typed Python wrappers for Vectorless result and graph types."""

from vectorless.types.graph import (
    DocumentGraphWrapper,
    EdgeEvidence,
    GraphEdge,
    GraphNode,
    WeightedKeyword,
)
from vectorless.types.results import (
    Evidence,
    FailedItem,
    IndexItemWrapper,
    IndexMetrics,
    IndexResultWrapper,
    QueryMetrics,
    QueryResponse,
    QueryResult,
)

__all__ = [
    # Results
    "Evidence",
    "FailedItem",
    "IndexItemWrapper",
    "IndexMetrics",
    "IndexResultWrapper",
    "QueryMetrics",
    "QueryResponse",
    "QueryResult",
    # Graph
    "DocumentGraphWrapper",
    "EdgeEvidence",
    "GraphEdge",
    "GraphNode",
    "WeightedKeyword",
]
