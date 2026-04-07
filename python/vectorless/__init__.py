"""
Vectorless - Hierarchical document intelligence without vectors.

A document intelligence engine that uses tree-based understanding
instead of vector databases for accurate, explainable retrieval.

Quick Start:
    from vectorless import Engine, IndexContext

    # Create engine
    engine = Engine(workspace="./data")

    # Index a document
    ctx = IndexContext.from_file("./report.pdf")
    doc_id = engine.index(ctx)

    # Query
    result = engine.query(doc_id, "What is the revenue?")
    print(result.content)
"""

from vectorless.vectorless import (
    Engine,
    IndexContext,
    QueryResult,
    DocumentInfo,
    VectorlessError,
    __version__,
)

__all__ = [
    "Engine",
    "IndexContext",
    "QueryResult",
    "DocumentInfo",
    "VectorlessError",
    "__version__",
]
