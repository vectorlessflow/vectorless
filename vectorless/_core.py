"""Internal re-exports from the Rust PyO3 module.

This module is NOT part of the public API.
The public Engine is ``vectorless.engine.Engine`` (Python strategy layer).
Here ``Engine`` refers to the raw Rust engine used internally for compile/document management.
"""

from vectorless._vectorless import (
    Answer,
    Concept,
    Config,
    DocumentGraph,
    DocumentGraphEdge,
    DocumentGraphNode,
    DocumentInfo,
    EdgeEvidence,
    Engine,
    Evidence,
    GraphEdge,
    LlmMetricsReport,
    MetricsReport,
    ReasoningTrace,
    RetrievalMetricsReport,
    TraceStep,
    VectorlessError,
    WeightedKeyword,
    __version__,
)

__all__ = [
    "Answer",
    "Concept",
    "Config",
    "DocumentGraph",
    "DocumentGraphEdge",
    "DocumentGraphNode",
    "DocumentInfo",
    "EdgeEvidence",
    "Engine",
    "Evidence",
    "GraphEdge",
    "LlmMetricsReport",
    "MetricsReport",
    "ReasoningTrace",
    "RetrievalMetricsReport",
    "TraceStep",
    "VectorlessError",
    "WeightedKeyword",
    "__version__",
]
