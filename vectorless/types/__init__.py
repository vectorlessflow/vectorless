"""Typed Python wrappers for Vectorless result and graph types."""

from vectorless.types.graph import (
    DocumentGraphWrapper,
    EdgeEvidence,
    GraphEdge,
    GraphNode,
    WeightedKeyword,
)
from vectorless.types.results import (
    CompileArtifact,
    CompileOutput,
    Evidence,
    FailedItem,
    IndexMetrics,
    QueryMetrics,
    QueryResponse,
    QueryResult,
)

__all__ = [
    # Results
    "CompileArtifact",
    "CompileOutput",
    "Evidence",
    "FailedItem",
    "IndexMetrics",
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
