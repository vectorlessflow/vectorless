"""
Vectorless — Reasoning-native document engine.

Every retrieval is a reasoning act.

Quick Start:
    from vectorless import Session

    session = Session(api_key="sk-...", model="gpt-4o")
    result = await session.index(path="./report.pdf")
    answer = await session.ask("What is the revenue?", doc_ids=[result.doc_id])
    print(answer.single().content)
"""

# High-level API (recommended)
from vectorless.session import Session
from vectorless.sync_session import SyncSession
from vectorless.config import EngineConfig, load_config, load_config_from_env, load_config_from_file
from vectorless.events import EventEmitter
from vectorless.streaming import StreamingQueryResult
from vectorless.types import (
    DocumentGraphWrapper,
    EdgeEvidence,
    Evidence,
    FailedItem,
    GraphEdge,
    GraphNode,
    IndexItemWrapper,
    IndexMetrics,
    IndexResultWrapper,
    QueryMetrics,
    QueryResponse,
    QueryResult,
    WeightedKeyword,
)

# Version and error types
from vectorless._vectorless import VectorlessError, __version__

__all__ = [
    # Primary API
    "Session",
    "SyncSession",
    # Configuration
    "EngineConfig",
    "load_config",
    "load_config_from_env",
    "load_config_from_file",
    # Events
    "EventEmitter",
    # Streaming
    "StreamingQueryResult",
    # Result types
    "QueryResponse",
    "QueryResult",
    "QueryMetrics",
    "Evidence",
    "IndexResultWrapper",
    "IndexItemWrapper",
    "IndexMetrics",
    "FailedItem",
    # Graph types
    "DocumentGraphWrapper",
    "GraphNode",
    "GraphEdge",
    "EdgeEvidence",
    "WeightedKeyword",
    # Error and version
    "VectorlessError",
    "__version__",
]
