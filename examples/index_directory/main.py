"""
Directory indexing example — recursively index all documents in a directory.

Usage:
    python index_directory.py /path/to/docs
    python index_directory.py /path/to/docs --no-recursive

Environment variables:
    LLM_API_KEY    — Your LLM API key (required)
    LLM_MODEL      — Model name (default: google/gemini-3-flash-preview)
    LLM_ENDPOINT   — API endpoint (default: http://localhost:4000/api/v1)
"""

import argparse
import asyncio
import os

from vectorless import Engine, IndexContext, QueryContext


async def main():
    parser = argparse.ArgumentParser(description="Index a directory of documents")
    parser.add_argument("directory", help="Directory path to index")
    parser.add_argument(
        "--no-recursive",
        action="store_true",
        help="Only scan top-level files (default: recursive)",
    )
    args = parser.parse_args()

    # Build engine
    api_key = os.environ.get("LLM_API_KEY", "sk-or-v1-...")
    model = os.environ.get("LLM_MODEL", "google/gemini-3-flash-preview")
    endpoint = os.environ.get("LLM_ENDPOINT", "http://localhost:4000/api/v1")

    engine = Engine(
        workspace="./workspace_directory_example",
        api_key=api_key,
        model=model,
        endpoint=endpoint,
    )

    recursive = not args.no_recursive

    # Index directory
    ctx = IndexContext.from_dir(args.directory, recursive=recursive)

    if ctx.is_empty():
        print(f"No supported files found in: {args.directory}")
        return

    print(f"{'Recursively scanning' if recursive else 'Scanning top-level files in'}: {args.directory}")
    print(f"Found files to index")

    result = await engine.index(ctx)

    print(f"\nIndexed {len(result.items)} document(s):")
    for item in result.items:
        print(f"  {item.name} ({item.doc_id})")
        if item.metrics:
            print(f"    nodes: {item.metrics.nodes_processed}, time: {item.metrics.total_time_ms}ms")

    if result.has_failures():
        print("\nFailed:")
        for f in result.failed:
            print(f"  {f.source} — {f.error}")

    # Query across all indexed documents
    query = "What is this about?"
    print(f'\nQuerying: "{query}"')

    answer = await engine.query(QueryContext(query))
    for item in answer.items:
        print(f"  [{item.doc_id} score={item.score:.2f}]")
        preview = item.content[:200]
        print(f"  {preview}")
        if len(item.content) > 200:
            print("  ...")

    # Metrics report
    report = engine.metrics_report()
    print("\nMetrics:")
    print(
        f"  LLM: {report.llm.total_calls} calls, "
        f"{report.llm.total_tokens} tokens, "
        f"${report.llm.estimated_cost_usd:.4f}"
    )
    print(
        f"  Retrieval: {report.retrieval.total_queries} queries, "
        f"avg score {report.retrieval.avg_path_score:.2f}"
    )

    # Cleanup
    docs = await engine.list()
    for doc in docs:
        await engine.remove(doc.id)


if __name__ == "__main__":
    asyncio.run(main())
