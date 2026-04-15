"""
PDF indexing example -- demonstrates indexing PDF files and inspecting metrics.

Usage:
    pip install vectorless
    python main.py [path/to/file.pdf]

If no path is given, uses the sample PDF in the repository.
"""

import asyncio
import os
import sys

from vectorless import (
    Engine,
    IndexContext,
    IndexItem,
    IndexMetrics,
    IndexOptions,
    QueryContext,
    VectorlessError,
)

# --- Configuration ---
API_KEY = os.environ.get("VECTORLESS_API_KEY", "sk-...")
MODEL = os.environ.get("VECTORLESS_MODEL", "gpt-4o")
ENDPOINT = os.environ.get("VECTORLESS_ENDPOINT", None)
# Resolve the sample PDF path relative to the repo root
SAMPLE_PDF = os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
    "samples",
    "Docker_Cheat_Sheet.pdf",
)


def print_separator(title: str) -> None:
    print(f"\n{'=' * 40}")
    print(f"  {title}")
    print(f"{'=' * 40}")


def print_metrics(item: IndexItem) -> None:
    """Pretty-print indexing metrics for a single item."""
    m: IndexMetrics | None = item.metrics
    if m is None:
        print("  (no metrics available)")
        return

    print(f"  Total time:       {m.total_time_ms:>6} ms")
    print(f"  Parse time:       {m.parse_time_ms:>6} ms")
    print(f"  Build time:       {m.build_time_ms:>6} ms")
    print(f"  Enhance time:     {m.enhance_time_ms:>6} ms")
    print(f"  Nodes processed:  {m.nodes_processed:>6}")
    print(f"  Summaries ok:     {m.summaries_generated:>6}")
    print(f"  Summaries failed: {m.summaries_failed:>6}")
    print(f"  LLM calls:        {m.llm_calls:>6}")
    print(f"  Tokens generated:  {m.total_tokens_generated:>6}")
    print(f"  Topics indexed:   {m.topics_indexed:>6}")
    print(f"  Keywords indexed: {m.keywords_indexed:>6}")


async def main() -> None:
    pdf_path = sys.argv[1] if len(sys.argv) > 1 else SAMPLE_PDF

    if not os.path.isfile(pdf_path):
        print(f"Error: file not found: {pdf_path}")
        sys.exit(1)

    engine = Engine(
        api_key=API_KEY,
        model=MODEL,
        endpoint=ENDPOINT,
    )

    # ---- Index with description + summaries enabled ----
    print_separator("Indexing PDF")

    options = IndexOptions(generate_summaries=True, generate_description=True)
    ctx = IndexContext.from_path(pdf_path).with_options(options)

    try:
        result = await engine.index(ctx)
    except VectorlessError as e:
        print(f"Indexing failed: [{e.kind}] {e.message}")
        return

    if result.has_failures():
        for f in result.failed:
            print(f"  Failed: {f.source} -- {f.error}")
        return

    doc_id = result.doc_id
    print(f"  doc_id: {doc_id}")

    for item in result.items:
        print(f"\n  Item: {item.name} ({item.format})")
        if item.page_count is not None:
            print(f"  Pages: {item.page_count}")
        if item.description:
            print(f"  Description: {item.description[:120]}...")
        print_metrics(item)

    # ---- Query the PDF ----
    print_separator("Query")

    answer = await engine.query(
        QueryContext("What is this document about?").with_doc_id(doc_id)
    )
    item = answer.single()
    if item:
        print(f"  Score:   {item.score:.2f}")
        print(f"  Nodes:   {item.node_ids}")
        print(f"  Content: {item.content[:300]}...")

    # ---- Cleanup ----
    print_separator("Cleanup")
    removed = await engine.clear()
    print(f"  Removed {removed} document(s)")


if __name__ == "__main__":
    asyncio.run(main())
