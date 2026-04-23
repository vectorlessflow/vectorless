"""tree command — visualize document tree structure."""

import asyncio
import os
from typing import Optional

import click

from vectorless.cli.workspace import get_workspace_path


def _create_engine(workspace_dir: str):
    """Create an Engine from workspace config."""
    from vectorless.engine import Engine

    config_path = os.path.join(workspace_dir, "config.toml")
    if os.path.exists(config_path):
        return Engine.from_config_file(config_path)
    return Engine.from_env()


def tree_cmd(
    doc_id: str,
    *,
    depth: Optional[int] = None,
    show_summary: bool = False,
    show_keywords: bool = False,
) -> None:
    """Visualize the hierarchical tree structure of an indexed document.

    Args:
        doc_id: Document identifier.
        depth: Max depth to display (None = full tree).
        show_summary: Include node summaries in output.
        show_keywords: Include routing keywords in output.

    Example output:
        API Guide (a1b2c3) — 45 nodes, 12 leaves
        1. Overview [routing: api-overview] (12 leaves)
        ├── 1.1 Introduction
        ├── 1.2 Authentication [keywords: auth, token, api-key]
        │   ├── 1.2.1 API Key Setup
        │   └── 1.2.2 OAuth Flow
        └── 1.3 Endpoints (18 leaves)
    """
    workspace = get_workspace_path()

    try:
        session = _create_engine(workspace)
    except Exception as e:
        raise click.ClickException(f"Failed to create engine: {e}") from e

    async def _run():
        graph = await session.get_graph()
        documents = await session.list_documents()
        return graph, documents

    try:
        graph, documents = asyncio.run(_run())
    except Exception as e:
        raise click.ClickException(f"Failed to retrieve document data: {e}") from e

    # Find the matching document from the list
    doc_info = None
    for d in documents:
        d_id = getattr(d, "doc_id", None) or getattr(d, "id", None)
        if d_id == doc_id:
            doc_info = d
            break

    if doc_info is None:
        raise click.ClickException(f"Document not found: {doc_id}")

    name = getattr(doc_info, "name", "Unknown")
    fmt = getattr(doc_info, "format", "unknown")
    metrics = getattr(doc_info, "metrics", None)

    # Display header
    node_count = metrics.nodes_processed if metrics else "?"
    click.echo(f"{name} ({doc_id})")
    click.echo(f"  Format: {fmt}")

    if metrics:
        click.echo(
            f"  Nodes: {metrics.nodes_processed}, "
            f"Summaries: {metrics.summaries_generated}, "
            f"Keywords: {metrics.keywords_indexed}"
        )

    click.echo("")

    # Since Rust tree is not directly exposed, show graph-based structure
    if graph and not graph.is_empty():
        node = graph.get_node(doc_id)
        if node:
            click.echo(f"  Graph node: {node.title}")
            click.echo(f"  Format: {node.format}, Node count: {node.node_count}")

            if show_keywords and node.top_keywords:
                kw_str = ", ".join(
                    f"{kw.keyword} ({kw.weight:.2f})" for kw in node.top_keywords[:10]
                )
                click.echo(f"  Top keywords: {kw_str}")

            neighbors = graph.get_neighbors(doc_id)
            if neighbors:
                click.echo("")
                click.echo(f"  Related documents ({len(neighbors)} connections):")
                for edge in neighbors:
                    weight_str = f"weight={edge.weight:.2f}"
                    evidence_str = ""
                    if edge.evidence:
                        evidence_str = (
                            f", shared_keywords={edge.evidence.shared_keyword_count}"
                            f", jaccard={edge.evidence.keyword_jaccard:.3f}"
                        )
                    click.echo(f"    -> {edge.target_doc_id} ({weight_str}{evidence_str})")

                    if show_keywords and edge.evidence:
                        shared = ", ".join(
                            kw for kw, _ in edge.evidence.shared_keywords[:5]
                        )
                        if shared:
                            click.echo(f"       shared: {shared}")
        else:
            click.echo(f"  (Document {doc_id} not found in graph)")
    else:
        click.echo("  (No graph data available)")
        click.echo("  The document tree is not directly accessible from the CLI.")
        click.echo("  Graph data will be populated as more documents are indexed.")

    if show_summary and metrics:
        click.echo("")
        click.echo(f"  Index summary:")
        click.echo(f"    Total time: {metrics.total_time_ms}ms")
        click.echo(f"    LLM calls: {metrics.llm_calls}")
        click.echo(f"    Tokens generated: {metrics.total_tokens_generated}")
