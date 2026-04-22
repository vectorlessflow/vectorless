"""
Session API walkthrough -- demonstrates the full high-level Vectorless API.

This example uses the Session class (recommended entry point) to cover:
  1. Session creation (constructor / from_env / from_config_file)
  2. Indexing from various sources (content, path, directory, bytes)
  3. Batch indexing with concurrency control
  4. Querying with doc_ids and workspace scope
  5. Streaming query with real-time events
  6. Document management (list, exists, remove, clear)
  7. Cross-document relationship graph
  8. Event callbacks for progress monitoring
  9. Metrics reporting
  10. SyncSession (synchronous API, no async/await)

Usage:
    export VECTORLESS_API_KEY="sk-..."
    export VECTORLESS_MODEL="gpt-4o"
    pip install vectorless
    python main.py
"""

import asyncio
import os
import tempfile

from vectorless import (
    Session,
    SyncSession,
    EventEmitter,
    VectorlessError,
)
from vectorless.events import IndexEventType, QueryEventType


# ──────────────────────────────────────────────────────────────────
#  Sample documents used throughout the example
# ──────────────────────────────────────────────────────────────────

ARCHITECTURE_DOC = """\
# Vectorless Architecture

## Overview

Vectorless is a reasoning-native document intelligence engine.
It uses hierarchical semantic trees instead of vector embeddings.

## Key Concepts

- **Semantic Tree**: Documents are parsed into a tree of sections.
- **LLM Navigation**: Queries are resolved by traversing the tree.
- **No Vectors**: No embeddings, no similarity search, no vector DB.

## Retrieval Flow

Engine.query()
  -> query/understand() -> QueryPlan
  -> Orchestrator dispatches Workers
  -> Workers navigate document trees
  -> rerank -> synthesis -> answer
"""

FINANCE_DOC = """\
# Q4 Financial Report

## Revenue

Total revenue for Q4 was $12.3M, up 15% from Q3.
SaaS subscriptions accounted for $8.1M, consulting for $4.2M.

## Costs

Operating costs were $9.8M, including $3.2M in engineering salaries.
Marketing spend was reduced by 8% to $1.5M.

## Outlook

Projected Q1 revenue is $13.5M based on current pipeline.
"""

SECURITY_DOC = """\
# Security Policy

## Authentication

All API requests require a Bearer token in the Authorization header.
Tokens expire after 24 hours and must be refreshed.

## Data Encryption

Data at rest is encrypted using AES-256. Data in transit uses TLS 1.3.

## Audit Logging

All access to sensitive data is logged and retained for 90 days.
"""


# ──────────────────────────────────────────────────────────────────
#  Helper: set up a temp directory with sample files
# ──────────────────────────────────────────────────────────────────

def create_sample_directory() -> tuple[str, list[str]]:
    """Create a temp directory with sample documents. Returns (dir, paths)."""
    tmpdir = tempfile.mkdtemp(prefix="vectorless_walkthrough_")
    docs = {
        "architecture.md": ARCHITECTURE_DOC,
        "finance.md": FINANCE_DOC,
        "security.md": SECURITY_DOC,
    }
    paths = []
    for name, content in docs.items():
        path = os.path.join(tmpdir, name)
        with open(path, "w") as f:
            f.write(content)
        paths.append(path)
    return tmpdir, paths


def cleanup_directory(tmpdir: str) -> None:
    """Remove all files in the temp directory."""
    for fname in os.listdir(tmpdir):
        os.remove(os.path.join(tmpdir, fname))
    os.rmdir(tmpdir)


# ──────────────────────────────────────────────────────────────────
#  Section 1: Session Creation
# ──────────────────────────────────────────────────────────────────

async def demo_session_creation() -> Session:
    """Demonstrate different ways to create a Session."""
    print("=" * 60)
    print("  1. Session Creation")
    print("=" * 60)

    # Option A: Constructor with explicit credentials
    api_key = os.environ.get("VECTORLESS_API_KEY", "sk-...")
    model = os.environ.get("VECTORLESS_MODEL", "gpt-4o")
    endpoint = os.environ.get("VECTORLESS_ENDPOINT")

    session = Session(api_key=api_key, model=model, endpoint=endpoint)
    print(f"  Created: {session}")

    # Option B: from environment variables
    # session = Session.from_env()

    # Option C: from a config file
    # session = Session.from_config_file("~/.vectorless/config.toml")

    # Option D: with an EventEmitter for progress callbacks
    # events = EventEmitter()
    # session = Session(api_key=api_key, model=model, events=events)

    print()
    return session


# ──────────────────────────────────────────────────────────────────
#  Section 2: Indexing from Various Sources
# ──────────────────────────────────────────────────────────────────

async def demo_indexing(session: Session, tmpdir: str, paths: list[str]) -> dict[str, str]:
    """Demonstrate indexing from content, path, directory, and bytes."""
    print("=" * 60)
    print("  2. Indexing")
    print("=" * 60)

    doc_ids: dict[str, str] = {}

    # --- 2a. Index from in-memory content ---
    print("  [content] Indexing from string...")
    result = await session.index(
        content=ARCHITECTURE_DOC,
        format="markdown",
        name="architecture",
    )
    doc_ids["architecture"] = result.doc_id  # type: ignore[assignment]
    print(f"    doc_id: {result.doc_id}")
    print(f"    items:  {result.total()}")

    # --- 2b. Index from a file path ---
    print("  [path]    Indexing from file path...")
    result = await session.index(path=paths[1], name="finance")
    doc_ids["finance"] = result.doc_id  # type: ignore[assignment]
    print(f"    doc_id: {result.doc_id}")

    # --- 2c. Index from raw bytes ---
    print("  [bytes]   Indexing from raw bytes...")
    result = await session.index(
        bytes_data=SECURITY_DOC.encode("utf-8"),
        format="markdown",
        name="security",
    )
    doc_ids["security"] = result.doc_id  # type: ignore[assignment]
    print(f"    doc_id: {result.doc_id}")

    # --- 2d. Index a directory ---
    print("  [dir]     Indexing a directory...")
    # Clear first to see fresh results
    await session.clear_all()

    result = await session.index(directory=tmpdir, name="all_docs")
    print(f"    doc_id: {result.doc_id}")
    print(f"    items:  {len(result.items)}")
    for item in result.items:
        print(f"      - {item.name} ({item.doc_id[:8]}...)")
        doc_ids[item.name] = item.doc_id

    print()
    return doc_ids


# ──────────────────────────────────────────────────────────────────
#  Section 3: Batch Indexing with Concurrency
# ──────────────────────────────────────────────────────────────────

async def demo_batch_indexing(session: Session, paths: list[str]) -> list[str]:
    """Demonstrate batch indexing with concurrent jobs."""
    print("=" * 60)
    print("  3. Batch Indexing (concurrency=2)")
    print("=" * 60)

    # Clear to start fresh
    await session.clear_all()

    results = await session.index_batch(
        paths,
        mode="default",
        jobs=2,       # max 2 concurrent indexing operations
        force=False,
    )

    doc_ids = []
    for r in results:
        print(f"    {r.doc_id[:8]}... ({len(r.items)} items)")
        for item in r.items:
            doc_ids.append(item.doc_id)

    print(f"  Batch indexed {len(results)} file(s), {len(doc_ids)} document(s) total")
    print()
    return doc_ids


# ──────────────────────────────────────────────────────────────────
#  Section 4: Querying
# ──────────────────────────────────────────────────────────────────

async def demo_querying(session: Session, doc_ids: list[str]) -> None:
    """Demonstrate querying with doc_ids and workspace scope."""
    print("=" * 60)
    print("  4. Querying")
    print("=" * 60)

    # --- Query specific documents ---
    print("  [ask] Query specific documents...")
    response = await session.ask(
        "What was the total revenue for Q4?",
        doc_ids=doc_ids[:2],  # limit to first two docs
    )

    result = response.single()
    if result:
        print(f"    Score:      {result.score:.2f}")
        print(f"    Confidence: {result.confidence:.2f}")
        print(f"    Answer:     {result.content[:150]}...")
        if result.evidence:
            print(f"    Evidence:   {len(result.evidence)} item(s)")
            for ev in result.evidence[:2]:
                print(f"      - {ev.title}: {ev.content[:80]}...")
        if result.metrics:
            print(f"    LLM calls:  {result.metrics.llm_calls}")
            print(f"    Nodes:      {result.metrics.nodes_visited}")

    # --- Query across all documents ---
    print()
    print("  [workspace_scope] Query across entire workspace...")
    response = await session.ask(
        "How is data encrypted?",
        workspace_scope=True,
    )
    for item in response.items:
        print(f"    [{item.doc_id[:8]}...] score={item.score:.2f}")
        print(f"      {item.content[:120]}...")

    # --- Query with timeout ---
    print()
    print("  [timeout] Query with 30s timeout...")
    try:
        response = await session.ask(
            "What is the retrieval flow?",
            doc_ids=doc_ids,
            timeout_secs=30,
        )
        if response.single():
            print(f"    Answer: {response.single().content[:150]}...")
    except VectorlessError as e:
        print(f"    Error: {e}")

    print()


# ──────────────────────────────────────────────────────────────────
#  Section 5: Streaming Query
# ──────────────────────────────────────────────────────────────────

async def demo_streaming(session: Session, doc_ids: list[str]) -> None:
    """Demonstrate streaming query with real-time events."""
    print("=" * 60)
    print("  5. Streaming Query")
    print("=" * 60)

    stream = await session.query_stream(
        "What are the key concepts?",
        doc_ids=doc_ids[:1],
    )

    event_count = 0
    async for event in stream:
        event_count += 1
        event_type = event.get("type", "unknown")
        # Print a compact summary of each event
        if event_type == "completed":
            results = event.get("results", [])
            print(f"    [{event_count}] completed — {len(results)} result(s)")
        elif event_type == "error":
            print(f"    [{event_count}] error — {event.get('message', '')}")
        else:
            print(f"    [{event_count}] {event_type}")

    # The final result is available after iteration completes
    if stream.result:
        final = stream.result
        item = final.single()
        if item:
            print(f"    Final answer: {item.content[:150]}...")

    print()


# ──────────────────────────────────────────────────────────────────
#  Section 6: Document Management
# ──────────────────────────────────────────────────────────────────

async def demo_document_management(session: Session, doc_ids: list[str]) -> None:
    """Demonstrate list, exists, remove, and clear."""
    print("=" * 60)
    print("  6. Document Management")
    print("=" * 60)

    # --- List all documents ---
    docs = await session.list_documents()
    print(f"  Listed {len(docs)} document(s):")
    for doc in docs:
        pages = f", pages={doc.page_count}" if doc.page_count else ""
        print(f"    {doc.name}  id={doc.id[:8]}...  format={doc.format}{pages}")

    # --- Check existence ---
    if doc_ids:
        exists = await session.document_exists(doc_ids[0])
        print(f"\n  exists({doc_ids[0][:8]}...): {exists}")

    # --- Remove a document ---
    if len(doc_ids) > 1:
        removed = await session.remove_document(doc_ids[1])
        print(f"  remove({doc_ids[1][:8]}...): {removed}")

        # Verify removal
        exists_after = await session.document_exists(doc_ids[1])
        print(f"  exists after removal: {exists_after}")

    # --- List again ---
    docs = await session.list_documents()
    print(f"\n  After removal: {len(docs)} document(s)")

    print()


# ──────────────────────────────────────────────────────────────────
#  Section 7: Cross-Document Relationship Graph
# ──────────────────────────────────────────────────────────────────

async def demo_graph(session: Session) -> None:
    """Demonstrate the cross-document relationship graph."""
    print("=" * 60)
    print("  7. Document Graph")
    print("=" * 60)

    graph = await session.get_graph()

    if graph is None or graph.is_empty():
        print("  Graph is empty (no documents or no relationships found)")
        print()
        return

    print(f"  Nodes: {graph.node_count()}, Edges: {graph.edge_count()}")

    for did in graph.doc_ids():
        node = graph.get_node(did)
        if node:
            keywords = ", ".join(k.keyword for k in node.top_keywords[:5])
            neighbors = graph.get_neighbors(did)
            print(f"  {node.title}")
            print(f"    format: {node.format}, nodes: {node.node_count}")
            print(f"    keywords: [{keywords}]")
            print(f"    neighbors: {len(neighbors)}")
            for edge in neighbors[:3]:
                target = graph.get_node(edge.target_doc_id)
                target_name = target.title if target else edge.target_doc_id[:8]
                weight_str = f"weight={edge.weight:.2f}"
                evidence_str = ""
                if edge.evidence:
                    evidence_str = f", shared_keywords={edge.evidence.shared_keyword_count}"
                print(f"      -> {target_name} ({weight_str}{evidence_str})")

    print()


# ──────────────────────────────────────────────────────────────────
#  Section 8: Event Callbacks
# ──────────────────────────────────────────────────────────────────

async def demo_events() -> None:
    """Demonstrate event callbacks with EventEmitter."""
    print("=" * 60)
    print("  8. Event Callbacks")
    print("=" * 60)

    events = EventEmitter()

    @events.on_index
    def on_index_event(event):
        if event.event_type == IndexEventType.STARTED:
            print(f"    [INDEX] Started: {event.path or event.message}")
        elif event.event_type == IndexEventType.COMPLETE:
            print(f"    [INDEX] Complete: {event.doc_id or event.message}")
        elif event.event_type == IndexEventType.ERROR:
            print(f"    [INDEX] Error: {event.message}")

    @events.on_query
    def on_query_event(event):
        if event.event_type == QueryEventType.STARTED:
            print(f"    [QUERY] Started: {event.query}")
        elif event.event_type == QueryEventType.COMPLETE:
            print(f"    [QUERY] Complete: {event.total_results} result(s)")

    # Create a session with the event emitter
    api_key = os.environ.get("VECTORLESS_API_KEY", "sk-...")
    model = os.environ.get("VECTORLESS_MODEL", "gpt-4o")
    session = Session(api_key=api_key, model=model, events=events)

    # Index and query — events fire automatically
    await session.index(content=ARCHITECTURE_DOC, format="markdown", name="demo_events")
    await session.ask("What are the key concepts?", workspace_scope=True)

    await session.clear_all()
    print()


# ──────────────────────────────────────────────────────────────────
#  Section 9: Metrics
# ──────────────────────────────────────────────────────────────────

async def demo_metrics(session: Session) -> None:
    """Demonstrate metrics reporting."""
    print("=" * 60)
    print("  9. Metrics Report")
    print("=" * 60)

    report = session.metrics_report()
    if report:
        # The report contains llm and retrieval subsections
        if hasattr(report, "llm"):
            llm = report.llm
            print(f"  LLM Metrics:")
            print(f"    Total calls:     {getattr(llm, 'total_calls', 'N/A')}")
            print(f"    Total tokens:    {getattr(llm, 'total_tokens', 'N/A')}")
            print(f"    Cache hit rate:  {getattr(llm, 'cache_hit_rate', 'N/A')}")
        if hasattr(report, "retrieval"):
            ret = report.retrieval
            print(f"  Retrieval Metrics:")
            print(f"    Total queries:   {getattr(ret, 'total_queries', 'N/A')}")
            print(f"    Avg latency:     {getattr(ret, 'avg_latency_ms', 'N/A')} ms")
    else:
        print("  No metrics available")

    print()


# ──────────────────────────────────────────────────────────────────
#  Section 10: SyncSession (Synchronous API)
# ──────────────────────────────────────────────────────────────────

def demo_sync_session() -> None:
    """Demonstrate the synchronous Session (no async/await needed)."""
    print("=" * 60)
    print("  10. SyncSession (no async/await)")
    print("=" * 60)

    api_key = os.environ.get("VECTORLESS_API_KEY", "sk-...")
    model = os.environ.get("VECTORLESS_MODEL", "gpt-4o")

    # Can also use: SyncSession.from_env()
    with SyncSession(api_key=api_key, model=model) as session:
        # Index from content
        result = session.index(
            content=FINANCE_DOC,
            format="markdown",
            name="sync_demo",
        )
        print(f"  Indexed: {result.doc_id}")

        # Query
        response = session.ask(
            "What was the total revenue?",
            doc_ids=[result.doc_id],  # type: ignore[list-item]
        )
        item = response.single()
        if item:
            print(f"  Answer: {item.content[:150]}...")

        # Cleanup
        session.clear_all()
        print("  Cleaned up")

    print()


# ──────────────────────────────────────────────────────────────────
#  Main
# ──────────────────────────────────────────────────────────────────

async def main() -> None:
    print()
    print("  Vectorless — Session API Walkthrough")
    print("  " + "-" * 38)
    print()

    # 1. Create session
    session = await demo_session_creation()

    # Set up sample directory
    tmpdir, paths = create_sample_directory()

    try:
        # 2. Indexing
        doc_id_map = await demo_indexing(session, tmpdir, paths)
        all_doc_ids = list(doc_id_map.values())

        # 3. Batch indexing (clears and re-indexes)
        batch_doc_ids = await demo_batch_indexing(session, paths)
        all_doc_ids = batch_doc_ids if batch_doc_ids else all_doc_ids

        # 4. Querying
        if all_doc_ids:
            await demo_querying(session, all_doc_ids)

        # 5. Streaming query
        if all_doc_ids:
            await demo_streaming(session, all_doc_ids)

        # 6. Document management
        await demo_document_management(session, all_doc_ids)

        # 7. Graph
        await demo_graph(session)

        # 8. Events (creates its own session)
        await demo_events()

        # 9. Metrics
        await demo_metrics(session)

    finally:
        # Cleanup
        await session.clear_all()
        cleanup_directory(tmpdir)
        print("=" * 60)
        print("  Cleanup complete.")
        print("=" * 60)

    # 10. SyncSession (separate, runs synchronously)
    demo_sync_session()

    print("  Done.")


if __name__ == "__main__":
    asyncio.run(main())
