"""
IndexMetrics example -- demonstrates inspecting detailed indexing pipeline metrics.

IndexMetrics exposes timing, node processing, LLM usage, and reasoning index
statistics for each indexed document.  This example compares two documents with
different IndexOptions to show how options affect the pipeline.

Usage:
    pip install vectorless
    python main.py
"""

import asyncio
import os

from vectorless import (
    Engine,
    IndexContext,
    IndexItem,
    IndexMetrics,
    IndexOptions,
    VectorlessError,
)

# --- Configuration ---
API_KEY = os.environ.get("VECTORLESS_API_KEY", "sk-...")
MODEL = os.environ.get("VECTORLESS_MODEL", "gpt-4o")
ENDPOINT = os.environ.get("VECTORLESS_ENDPOINT", None)
# --- Sample documents with varying complexity ---
SIMPLE_DOC = """\
# Quick Note

This is a short note about caching strategies.
Redis is commonly used as an in-memory cache.
"""

COMPLEX_DOC = """\
# Distributed Systems Design Guide

## Consensus

Raft is a consensus algorithm designed to be easy to understand.
It elects a leader via randomized timeouts and replicates log entries
to a majority of followers before committing them.

## Replication

State machine replication ensures that all replicas execute the same
commands in the same order. Primary-backup replication is simpler but
provides lower availability during leader failover.

## Partitioning

Consistent hashing distributes keys across nodes with minimal
remapping when the cluster size changes. Virtual nodes improve balance
when the key space is small.

## Failure Detection

Phi accrual failure detection treats failure as a continuous suspicion
level rather than a binary alive/dead state. This reduces false
positives during transient network issues.
"""


def print_pipeline_breakdown(m: IndexMetrics) -> None:
    """Print a breakdown of pipeline stages and their percentages."""
    total = m.total_time_ms
    if total == 0:
        print("    (no timing data)")
        return

    parse_pct = m.parse_time_ms / total * 100
    build_pct = m.build_time_ms / total * 100
    enhance_pct = m.enhance_time_ms / total * 100
    other_pct = max(0, 100 - parse_pct - build_pct - enhance_pct)

    print(f"    Parse:    {m.parse_time_ms:>5} ms  ({parse_pct:5.1f}%)")
    print(f"    Build:    {m.build_time_ms:>5} ms  ({build_pct:5.1f}%)")
    print(f"    Enhance:  {m.enhance_time_ms:>5} ms  ({enhance_pct:5.1f}%)")
    print(f"    Other:    {total - m.parse_time_ms - m.build_time_ms - m.enhance_time_ms:>5} ms  ({other_pct:5.1f}%)")


def print_llm_stats(m: IndexMetrics) -> None:
    """Print LLM utilization statistics."""
    print(f"    LLM calls:         {m.llm_calls}")
    print(f"    Tokens generated:   {m.total_tokens_generated}")
    if m.llm_calls > 0:
        avg_tokens = m.total_tokens_generated / m.llm_calls
        print(f"    Avg tokens/call:    {avg_tokens:.0f}")


def print_summary_stats(m: IndexMetrics) -> None:
    """Print summary generation success/failure."""
    total = m.summaries_generated + m.summaries_failed
    print(f"    Summaries ok:       {m.summaries_generated}")
    print(f"    Summaries failed:   {m.summaries_failed}")
    if total > 0:
        success_rate = m.summaries_generated / total * 100
        print(f"    Success rate:       {success_rate:.1f}%")


def print_reasoning_index(m: IndexMetrics) -> None:
    """Print reasoning index statistics."""
    print(f"    Nodes processed:    {m.nodes_processed}")
    print(f"    Topics indexed:     {m.topics_indexed}")
    print(f"    Keywords indexed:   {m.keywords_indexed}")


def print_full_report(item: IndexItem) -> None:
    """Print a full metrics report for an indexed item."""
    m = item.metrics
    print(f"  Document: {item.name} ({item.format})")
    if m is None:
        print("    (no metrics)")
        return

    print(f"  Total time: {m.total_time_ms} ms")
    print(f"  repr: {repr(m)}")

    print()
    print("  Pipeline stages:")
    print_pipeline_breakdown(m)

    print()
    print("  LLM usage:")
    print_llm_stats(m)

    print()
    print("  Summary generation:")
    print_summary_stats(m)

    print()
    print("  Reasoning index:")
    print_reasoning_index(m)


async def main() -> None:
    engine = Engine(
        api_key=API_KEY,
        model=MODEL,
        endpoint=ENDPOINT,
    )

    # ================================================================
    # 1. Index a simple document WITHOUT summaries
    # ================================================================
    print("=" * 55)
    print("  Run 1: Simple doc, summaries OFF")
    print("=" * 55)

    opts_no_summary = IndexOptions(
        generate_summaries=False,
        generate_description=False,
    )
    result = await engine.index(
        IndexContext.from_content(SIMPLE_DOC, "markdown")
        .with_name("simple_no_summary")
        .with_options(opts_no_summary)
    )
    item = result.items[0]
    print_full_report(item)
    doc_id_1 = item.doc_id
    print()

    # ================================================================
    # 2. Index the same simple document WITH summaries
    # ================================================================
    print("=" * 55)
    print("  Run 2: Simple doc, summaries ON")
    print("=" * 55)

    opts_with_summary = IndexOptions(
        generate_summaries=True,
        generate_description=True,
    )
    result = await engine.index(
        IndexContext.from_content(SIMPLE_DOC, "markdown")
        .with_name("simple_with_summary")
        .with_options(opts_with_summary)
    )
    item = result.items[0]
    print_full_report(item)
    doc_id_2 = item.doc_id
    print()

    # ================================================================
    # 3. Compare: summaries OFF vs ON for the simple doc
    # ================================================================
    m_off = (await engine.list())[0]  # first indexed
    # Find the second document's metrics via a fresh index
    # (We already have both items above; let's compare directly)

    # ================================================================
    # 4. Index a complex document WITH summaries
    # ================================================================
    print("=" * 55)
    print("  Run 3: Complex doc, summaries ON")
    print("=" * 55)

    result = await engine.index(
        IndexContext.from_content(COMPLEX_DOC, "markdown")
        .with_name("complex_with_summary")
        .with_options(opts_with_summary)
    )
    item = result.items[0]
    print_full_report(item)
    doc_id_3 = item.doc_id
    print()

    # ================================================================
    # 5. Summary table
    # ================================================================
    print("=" * 55)
    print("  Comparison table")
    print("=" * 55)

    docs = await engine.list()
    for doc in docs:
        print(f"  {doc.name:<30} id={doc.id[:8]}...")
        if doc.description:
            print(f"    description: {doc.description[:80]}")

    # ================================================================
    # Cleanup
    # ================================================================
    print()
    cleared = await engine.clear()
    print(f"Cleaned up {cleared} document(s).")


if __name__ == "__main__":
    asyncio.run(main())
