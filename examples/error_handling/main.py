"""
Error handling example -- demonstrates catching and inspecting VectorlessError.

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

async def main() -> None:
    engine = Engine(
        api_key=API_KEY,
        model=MODEL,
        endpoint=ENDPOINT,
    )

    # ---- 1. Invalid format ----
    print("--- Invalid format in from_bytes ---")
    try:
        ctx = IndexContext.from_bytes(b"hello", "xml")
    except VectorlessError as e:
        print(f"  Caught VectorlessError:")
        print(f"    kind:    {e.kind}")
        print(f"    message: {e.message}")
        print(f"    repr:    {repr(e)}")
    print()

    # ---- 2. Invalid indexing mode ----
    print("--- Invalid indexing mode ---")
    try:
        opts = IndexOptions(mode="bad_mode")
    except VectorlessError as e:
        print(f"  Caught VectorlessError:")
        print(f"    kind:    {e.kind}")
        print(f"    message: {e.message}")
    print()

    # ---- 3. Query a non-existent document ----
    print("--- Query non-existent document ---")
    try:
        await engine.query(
            QueryContext("What is this?").with_doc_ids(["does-not-exist"])
        )
    except VectorlessError as e:
        print(f"  Caught VectorlessError:")
        print(f"    kind:    {e.kind}")
        print(f"    message: {e.message}")
    print()

    # ---- 4. Index with partial failure in batch ----
    print("--- Batch indexing with mixed results ---")
    good = IndexContext.from_content("# Real Doc\n\nThis is valid content.", "markdown")

    result = await engine.index(good.with_name("good_doc"))
    if result.has_failures():
        for f in result.failed:
            print(f"  Failed: {f.source} -- {f.error}")
    else:
        print(f"  Success: {result.doc_id}")

        # Inspect individual items
        for item in result.items:
            print(f"  Item: {item.name} ({item.format})")
            if item.metrics:
                m = item.metrics
                print(f"    Total time: {m.total_time_ms} ms, LLM calls: {m.llm_calls}")
    print()

    # ---- 5. Engine creation with bad credentials ----
    print("--- Engine with invalid credentials ---")
    try:
        bad_engine = Engine(
            api_key="sk-invalid-key-12345",
            model="gpt-4o",
        )
        # Try to use it -- the error will surface on the first LLM call
        await bad_engine.index(
            IndexContext.from_content("# Test\n", "markdown").with_name("fail_test")
        )
    except VectorlessError as e:
        print(f"  Caught VectorlessError:")
        print(f"    kind:    {e.kind}")
        print(f"    message: {e.message[:120]}...")
    print()

    # ---- Cleanup ----
    await engine.clear()
    print("Done.")


if __name__ == "__main__":
    asyncio.run(main())
