"""
Vectorless — Document Understanding Engine for AI.

Quick Start:
    from vectorless import Engine

    engine = Engine(api_key="sk-...", model="gpt-4o")
    doc = await engine.compile("./report.pdf")
    result = await engine.ask("What is the revenue?", doc_ids=[doc.doc_id])
    print(result.answer)
"""

# Primary API — Python Engine wrapping Rust compile + Python strategy
from vectorless.engine import Engine

# Configuration utilities
from vectorless.config import EngineConfig, load_config, load_config_from_env, load_config_from_file

# Events
from vectorless.events import EventEmitter

# Rust types re-exported for convenience
from vectorless._vectorless import (
    Answer,
    CollectedEvidence,
    Concept,
    Config,
    DocumentGraph,
    DocumentGraphNode,
    DocumentInfo,
    EdgeEvidence,
    Evidence,
    FindResult,
    GraphEdge,
    LlmMetricsReport,
    MatchResult,
    MetricsReport,
    NodeInfo,
    ReasoningTrace,
    RetrievalMetricsReport,
    SectionSummary,
    TraceStep,
    TopicEntry,
    VectorlessError,
    WeightedKeyword,
    WordCount,
    __version__,
)

__all__ = [
    # Primary API
    "Engine",
    # Configuration
    "EngineConfig",
    "load_config",
    "load_config_from_env",
    "load_config_from_file",
    "Config",
    # Events
    "EventEmitter",
    # Document types
    "DocumentInfo",
    "Concept",
    "NodeInfo",
    "MatchResult",
    "FindResult",
    "WordCount",
    "CollectedEvidence",
    "TopicEntry",
    "SectionSummary",
    # Answer types
    "Answer",
    "Evidence",
    "ReasoningTrace",
    "TraceStep",
    # Graph types
    "DocumentGraph",
    "DocumentGraphNode",
    "GraphEdge",
    "EdgeEvidence",
    "WeightedKeyword",
    # Metrics
    "LlmMetricsReport",
    "RetrievalMetricsReport",
    "MetricsReport",
    # Error and version
    "VectorlessError",
    "__version__",
]
