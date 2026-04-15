"""
Vectorless - Reasoning-native document intelligence engine for AI.

An ultra-performant reasoning-native document intelligence engine
that transforms documents into rich semantic trees and uses LLMs to
intelligently traverse the hierarchy for accurate, explainable retrieval.

Quick Start:
    from vectorless import Engine, IndexContext, QueryContext

    # Create engine
    engine = Engine(api_key="sk-...", model="gpt-4o")

    # Index a document
    ctx = IndexContext.from_path("./report.pdf")
    result = await engine.index(ctx)
    doc_id = result.doc_id

    # Query
    answer = await engine.query(QueryContext("What is the revenue?").with_doc_ids([doc_id]))
    print(answer.single().content)
"""

from vectorless._vectorless import (
    Engine,
    IndexContext,
    IndexOptions,
    IndexResult,
    IndexItem,
    IndexMetrics,
    QueryContext,
    QueryResult,
    QueryResultItem,
    DocumentInfo,
    DocumentGraph,
    DocumentGraphNode,
    GraphEdge,
    EdgeEvidence,
    WeightedKeyword,
    FailedItem,
    VectorlessError,
    __version__,
)

__all__ = [
    "Engine",
    "IndexContext",
    "IndexOptions",
    "IndexResult",
    "IndexItem",
    "IndexMetrics",
    "QueryContext",
    "QueryResult",
    "QueryResultItem",
    "DocumentInfo",
    "DocumentGraph",
    "DocumentGraphNode",
    "GraphEdge",
    "EdgeEvidence",
    "WeightedKeyword",
    "FailedItem",
    "VectorlessError",
    "__version__",
]
