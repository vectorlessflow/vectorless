"""info command — show document index details."""

import asyncio
import os

import click

from vectorless.cli.workspace import get_workspace_path
from vectorless.cli.output import format_json


def _create_session(workspace_dir: str):
    """Create a Session from workspace config."""
    from vectorless.session import Session

    config_path = os.path.join(workspace_dir, "config.toml")
    if os.path.exists(config_path):
        return Session.from_config_file(config_path)
    return Session.from_env()


def info_cmd(doc_id: str) -> None:
    """Show detailed information about an indexed document.

    Args:
        doc_id: Document identifier.

    Uses:
        Engine.list() -> filter by doc_id
        Display: title, source, format, node count, depth, leaf count,
                 total tokens, routing keywords, top-level sections,
                 indexed timestamp.

    Example output:
        Document: API Guide (a1b2c3)
        Source: ./docs/api-guide.md
        Format: Markdown
        Tree: 45 nodes, depth 4, 12 leaves
        Total tokens: 8,234
        Routing keywords: api, authentication, endpoints, rate-limit
        Top-level sections:
          1. Overview (12 leaves)
          2. Authentication (8 leaves)
          3. Endpoints (18 leaves)
    """
    workspace = get_workspace_path()

    try:
        session = _create_session(workspace)
    except Exception as e:
        raise click.ClickException(f"Failed to create session: {e}") from e

    async def _run():
        return await session.list_documents()

    try:
        documents = asyncio.run(_run())
    except Exception as e:
        raise click.ClickException(f"Failed to list documents: {e}") from e

    # Find matching document by doc_id
    doc = None
    for d in documents:
        d_id = getattr(d, "doc_id", None) or getattr(d, "id", None)
        if d_id == doc_id:
            doc = d
            break

    if doc is None:
        raise click.ClickException(f"Document not found: {doc_id}")

    # Display document details
    name = getattr(doc, "name", "Unknown")
    source = getattr(doc, "source_path", None)
    fmt = getattr(doc, "format", "unknown")
    pages = getattr(doc, "page_count", None)
    description = getattr(doc, "description", None)
    metrics = getattr(doc, "metrics", None)

    click.echo(f"Document: {name} ({doc_id})")
    if source:
        click.echo(f"Source: {source}")
    click.echo(f"Format: {fmt}")
    if pages:
        click.echo(f"Pages: {pages}")
    if description:
        click.echo(f"Description: {description}")

    if metrics:
        click.echo(f"Tree: {metrics.nodes_processed} nodes")
        click.echo(f"Summaries generated: {metrics.summaries_generated}")
        click.echo(f"LLM calls: {metrics.llm_calls}")
        click.echo(f"Total tokens: {metrics.total_tokens_generated}")
        click.echo(f"Topics indexed: {metrics.topics_indexed}")
        click.echo(f"Keywords indexed: {metrics.keywords_indexed}")
        click.echo(f"Indexing time: {metrics.total_time_ms}ms")
        click.echo(f"  Parse: {metrics.parse_time_ms}ms")
        click.echo(f"  Build: {metrics.build_time_ms}ms")
        click.echo(f"  Enhance: {metrics.enhance_time_ms}ms")
