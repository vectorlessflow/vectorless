"""stats command — workspace statistics."""

import asyncio
import os
from pathlib import Path

import click

from vectorless.cli.workspace import get_workspace_path, get_data_dir, get_cache_dir


def _create_session(workspace_dir: str):
    """Create a Session from workspace config."""
    from vectorless.session import Session

    config_path = os.path.join(workspace_dir, "config.toml")
    if os.path.exists(config_path):
        return Session.from_config_file(config_path)
    return Session.from_env()


def _dir_size(path: str) -> int:
    """Calculate total size of a directory in bytes."""
    total = 0
    try:
        for entry in Path(path).rglob("*"):
            if entry.is_file():
                total += entry.stat().st_size
    except OSError:
        pass
    return total


def _format_size(size_bytes: int) -> str:
    """Format bytes as human-readable size."""
    for unit in ("B", "KB", "MB", "GB"):
        if size_bytes < 1024:
            return f"{size_bytes:.1f} {unit}"
        size_bytes /= 1024
    return f"{size_bytes:.1f} TB"


def stats_cmd() -> None:
    """Show workspace statistics.

    Displays:
        - Workspace path
        - Number of indexed documents
        - Total nodes / leaves / tokens
        - Index size on disk
        - DocumentGraph info (edges, connected components)
        - Last indexed timestamp

    Uses:
        Engine.list() -> count documents
        Engine.metrics_report()
        Filesystem scan for size info
    """
    workspace = get_workspace_path()

    try:
        session = _create_session(workspace)
    except Exception as e:
        raise click.ClickException(f"Failed to create session: {e}") from e

    async def _run():
        documents = await session.list_documents()
        graph = await session.get_graph()
        return documents, graph

    try:
        documents, graph = asyncio.run(_run())
    except Exception as e:
        raise click.ClickException(f"Failed to retrieve workspace data: {e}") from e

    # Compute aggregate stats
    total_nodes = 0
    total_summaries = 0
    total_tokens = 0
    total_llm_calls = 0
    total_keywords = 0
    total_topics = 0

    for doc in documents:
        metrics = getattr(doc, "metrics", None)
        if metrics:
            total_nodes += metrics.nodes_processed
            total_summaries += metrics.summaries_generated
            total_tokens += metrics.total_tokens_generated
            total_llm_calls += metrics.llm_calls
            total_keywords += metrics.keywords_indexed
            total_topics += metrics.topics_indexed

    # Calculate disk usage
    data_size = _dir_size(get_data_dir(workspace))
    cache_size = _dir_size(get_cache_dir(workspace))

    # Display stats
    click.echo(f"Workspace: {workspace}")
    click.echo(f"Documents indexed: {len(documents)}")
    click.echo("")

    if documents:
        click.echo("Index statistics:")
        click.echo(f"  Total nodes: {total_nodes}")
        click.echo(f"  Total summaries: {total_summaries}")
        click.echo(f"  Total tokens generated: {total_tokens:,}")
        click.echo(f"  Total LLM calls (indexing): {total_llm_calls}")
        click.echo(f"  Total keywords indexed: {total_keywords}")
        click.echo(f"  Total topics indexed: {total_topics}")

    click.echo("")
    click.echo("Disk usage:")
    click.echo(f"  Data: {_format_size(data_size)}")
    click.echo(f"  Cache: {_format_size(cache_size)}")

    # Graph info
    if graph and not graph.is_empty():
        click.echo("")
        click.echo("Document graph:")
        click.echo(f"  Nodes: {graph.node_count()}")
        click.echo(f"  Edges: {graph.edge_count()}")

        doc_ids = graph.doc_ids()
        if doc_ids:
            click.echo(f"  Connected documents: {', '.join(doc_ids)}")
    else:
        click.echo("")
        click.echo("Document graph: (empty)")

    # Engine metrics
    try:
        report = session.metrics_report()
        if report:
            click.echo("")
            click.echo(f"Engine metrics: {report}")
    except Exception:
        pass
