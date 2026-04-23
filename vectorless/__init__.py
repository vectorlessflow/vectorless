"""
Vectorless — Document Understanding Engine for AI.

Quick Start:
    from vectorless import Engine

    engine = Engine(api_key="sk-...", model="gpt-4o")
    doc = await engine.index("./report.pdf")
    answer = await engine.ask("What is the revenue?", doc_ids=[doc.doc_id])
    print(answer.single().content)
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
    Concept,
    Config,
    DocumentGraph,
    DocumentInfo,
    EdgeEvidence,
    Evidence,
    GraphEdge,
    MetricsReport,
    ReasoningTrace,
    TraceStep,
    VectorlessError,
    WeightedKeyword,
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
    # Answer types
    "Answer",
    "Evidence",
    "ReasoningTrace",
    "TraceStep",
    # Graph types
    "DocumentGraph",
    "GraphEdge",
    "EdgeEvidence",
    "WeightedKeyword",
    # Metrics
    "MetricsReport",
    # Error and version
    "VectorlessError",
    "__version__",
]
