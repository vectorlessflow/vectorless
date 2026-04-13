"""
Document management example -- demonstrates CRUD operations on indexed documents:
list, exists, remove, and clear.

Usage:
    pip install vectorless
    python main.py
"""

import asyncio
import os

from vectorless import (
    Engine,
    IndexContext,
    QueryContext,
    VectorlessError,
)

# --- Configuration ---
API_KEY = os.environ.get("VECTORLESS_API_KEY", "sk-...")
MODEL = os.environ.get("VECTORLESS_MODEL", "gpt-4o")
ENDPOINT = os.environ.get("VECTORLESS_ENDPOINT", None)
WORKSPACE = "./workspace"

# Sample documents
SAMPLE_A = """\
# Project Alpha

## Overview

Project Alpha is a next-generation database engine written in Rust.
It supports ACID transactions and serializable isolation.

## Features

- MVCC concurrency control
- B-tree and LSM storage engines
- Query planner with cost-based optimization
"""

SAMPLE_B = """\
# Project Beta

## Overview

Project Beta is a web framework for building real-time applications.
It uses WebSocket-based communication and server-side rendering.

## Features

- Hot module reloading
- Built-in authentication middleware
- Automatic code splitting
"""


async def main() -> None:
    engine = Engine(
        workspace=WORKSPACE,
        api_key=API_KEY,
        model=MODEL,
        endpoint=ENDPOINT,
    )

    # ---- Index two documents ----
    print("Indexing two documents...")

    result_a = await engine.index(
        IndexContext.from_content(SAMPLE_A, "markdown").with_name("alpha")
    )
    doc_id_a = result_a.doc_id
    print(f"  A: {doc_id_a}")

    result_b = await engine.index(
        IndexContext.from_content(SAMPLE_B, "markdown").with_name("beta")
    )
    doc_id_b = result_b.doc_id
    print(f"  B: {doc_id_b}")
    print()

    # ---- list() -- show all indexed documents ----
    print("--- list() ---")
    docs = await engine.list()
    for doc in docs:
        pages = f", pages={doc.page_count}" if doc.page_count else ""
        lines = f", lines={doc.line_count}" if doc.line_count else ""
        print(f"  {doc.name}  id={doc.id[:8]}...  format={doc.format}{pages}{lines}")
    print(f"  Total: {len(docs)} document(s)\n")

    # ---- exists() -- check if a document is indexed ----
    print("--- exists() ---")
    for did, label in [(doc_id_a, "A"), (doc_id_b, "B"), ("nonexistent-id", "?")]:
        found = await engine.exists(did)
        print(f"  {label}: exists={found}")
    print()

    # ---- Query a specific document ----
    print("--- query(doc_id_a) ---")
    answer = await engine.query(
        QueryContext("What storage engines does Alpha support?").with_doc_id(doc_id_a)
    )
    item = answer.single()
    if item:
        print(f"  Score: {item.score:.2f}")
        print(f"  Answer: {item.content[:200]}...\n")

    # ---- remove() -- delete a single document ----
    print("--- remove(doc_id_a) ---")
    removed = await engine.remove(doc_id_a)
    print(f"  Removed A: {removed}")

    # Verify it's gone
    exists_a = await engine.exists(doc_id_a)
    print(f"  exists(A) after removal: {exists_a}")
    print()

    # ---- list() again -- only B should remain ----
    print("--- list() after removal ---")
    docs = await engine.list()
    for doc in docs:
        print(f"  {doc.name}  id={doc.id[:8]}...")
    print(f"  Total: {len(docs)} document(s)\n")

    # ---- clear() -- remove all remaining documents ----
    print("--- clear() ---")
    cleared = await engine.clear()
    print(f"  Cleared {cleared} document(s)")

    docs = await engine.list()
    print(f"  Remaining: {len(docs)} document(s)")


if __name__ == "__main__":
    asyncio.run(main())
