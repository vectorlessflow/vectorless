"""
Batch indexing example -- demonstrates indexing multiple documents at once
using from_paths, from_dir, and from_bytes.

Usage:
    pip install vectorless
    python main.py
"""

import asyncio
import os

from vectorless import (
    Engine,
    IndexContext,
    IndexOptions,
    QueryContext,
    VectorlessError,
)

# --- Configuration ---
API_KEY = os.environ.get("VECTORLESS_API_KEY", "sk-...")
MODEL = os.environ.get("VECTORLESS_MODEL", "gpt-4o")
ENDPOINT = os.environ.get("VECTORLESS_ENDPOINT", None)
# Sample documents for demonstration
DOCS = {
    "alpha.md": """\
# Alpha Report

## Summary

Alpha is a distributed key-value store designed for low-latency reads.
It uses a log-structured merge tree for storage.

## Architecture

Write requests go through a write-ahead log, then are buffered in memory.
When the buffer is full, it is flushed to disk as an immutable SSTable.
""",
    "beta.md": """\
# Beta Report

## Summary

Beta is a stream processing engine that consumes events from Kafka topics
and applies real-time transformations using a DAG-based execution model.

## Performance

Beta processes up to 2 million events per second per node on commodity hardware.
""",
    "gamma.md": """\
# Gamma Report

## Summary

Gamma is a feature store that bridges the gap between offline feature
computation and online serving. Features are computed in Spark and served
via a low-latency gRPC endpoint.

## Integration

Gamma integrates with Alpha for feature metadata storage and Beta for
real-time feature updates.
""",
}


def write_sample_docs(base_dir: str) -> list[str]:
    """Write sample markdown files and return their paths."""
    paths = []
    for name, content in DOCS.items():
        path = os.path.join(base_dir, name)
        with open(path, "w") as f:
            f.write(content)
        paths.append(path)
    return paths


async def main() -> None:
    engine = Engine(
        api_key=API_KEY,
        model=MODEL,
        endpoint=ENDPOINT,
    )

    # Create a temp directory with sample documents
    docs_dir = "./batch_docs"
    os.makedirs(docs_dir, exist_ok=True)
    paths = write_sample_docs(docs_dir)

    # ---- 1. Index multiple files at once via from_paths ----
    print("=" * 50)
    print("  from_paths -- index a list of files")
    print("=" * 50)

    ctx = IndexContext.from_paths(paths)
    result = await engine.index(ctx)

    print(f"  Indexed {len(result.items)} document(s)")
    for item in result.items:
        print(f"    - {item.name} ({item.doc_id[:8]}...)")
    if result.has_failures():
        for f in result.failed:
            print(f"    ! Failed: {f.source} -- {f.error}")
    print()

    doc_ids = [item.doc_id for item in result.items]

    # ---- 2. Query across all batch-indexed documents ----
    print("=" * 50)
    print("  Query across multiple documents")
    print("=" * 50)

    answer = await engine.query(
        QueryContext(
            "Which system processes the most events per second?"
        ).with_doc_ids(doc_ids)
    )
    for item in answer.items:
        print(f"  [{item.doc_id[:8]}...] score={item.score:.2f}")
        print(f"    {item.content[:200]}...")
    print()

    # ---- 3. Index a directory via from_dir ----
    print("=" * 50)
    print("  from_dir -- index all supported files in a directory")
    print("=" * 50)

    # Clear first so we see fresh results
    await engine.clear()

    ctx = IndexContext.from_dir(docs_dir).with_options(
        IndexOptions(generate_summaries=True, generate_description=True)
    )
    result = await engine.index(ctx)

    print(f"  Indexed {len(result.items)} document(s)")
    for item in result.items:
        desc = item.description[:80] if item.description else "N/A"
        print(f"    - {item.name}: {desc}...")
    print()

    # ---- 4. Index from raw bytes via from_bytes ----
    print("=" * 50)
    print("  from_bytes -- index in-memory content")
    print("=" * 50)

    md_bytes = b"""# Delta Notes

## Key Points

- Delta uses CRDTs for conflict-free replication.
- Writes are locally committed then asynchronously propagated.
- Read repair ensures eventual consistency across all replicas.
"""

    ctx = IndexContext.from_bytes(md_bytes, "markdown").with_name("delta")
    result = await engine.index(ctx)

    print(f"  Indexed: {result.doc_id}")
    print()

    # ---- Cleanup ----
    print("=" * 50)
    print("  Cleanup")
    print("=" * 50)

    removed = await engine.clear()
    print(f"  Removed {removed} document(s)")

    # Remove temp files
    for p in paths:
        os.remove(p)
    os.rmdir(docs_dir)
    print(f"  Cleaned up {docs_dir}/")


if __name__ == "__main__":
    asyncio.run(main())
