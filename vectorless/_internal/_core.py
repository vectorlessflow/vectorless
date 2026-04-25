"""Internal re-exports from the Rust PyO3 module.

This module is NOT part of the public API.
The public Engine is ``vectorless.engine.Engine`` (Python strategy layer).
Here ``Engine`` refers to the raw Rust engine used internally for compile/document management.
"""

from vectorless._vectorless import (
    CollectedEvidence,
    Concept,
    Config,
    DocumentGraph,
    DocumentGraphNode,
    DocumentInfo,
    EdgeEvidence,
    Engine,
    FindResult,
    GraphEdge,
    LlmMetricsReport,
    MatchResult,
    MetricsReport,
    NodeInfo,
    RetrievalMetricsReport,
    SectionSummary,
    TopicEntry,
    VectorlessError,
    WeightedKeyword,
    WordCount,
    __version__,
)

__all__ = [
    "CollectedEvidence",
    "Concept",
    "Config",
    "DocumentGraph",
    "DocumentGraphNode",
    "DocumentInfo",
    "EdgeEvidence",
    "Engine",
    "FindResult",
    "GraphEdge",
    "LlmMetricsReport",
    "MatchResult",
    "MetricsReport",
    "NodeInfo",
    "RetrievalMetricsReport",
    "SectionSummary",
    "TopicEntry",
    "VectorlessError",
    "WeightedKeyword",
    "WordCount",
    "__version__",
]
