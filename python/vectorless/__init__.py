"""
Vectorless - Hierarchical document intelligence without vectors.

A document intelligence engine that uses tree-based understanding
instead of vector databases for accurate, explainable retrieval.

Quick Start:
    from vectorless import Engine, IndexContext, QueryContext

    # Create engine
    engine = Engine(workspace="./data", api_key="sk-...", model="gpt-4o")

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
    StrategyPreference,
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
    "StrategyPreference",
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
