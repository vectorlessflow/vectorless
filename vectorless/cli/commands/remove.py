"""remove command — remove document index."""

import asyncio
import os

import click

from vectorless.cli.workspace import get_workspace_path


def _create_engine(workspace_dir: str):
    """Create an Engine from workspace config."""
    from vectorless.engine import Engine

    config_path = os.path.join(workspace_dir, "config.toml")
    if os.path.exists(config_path):
        return Engine.from_config_file(config_path)
    return Engine.from_env()


def remove_cmd(doc_id: str) -> None:
    """Remove a document from the index.

    Args:
        doc_id: Document identifier to remove.

    Uses:
        Engine.remove(doc_id)
    """
    workspace = get_workspace_path()

    try:
        session = _create_engine(workspace)
    except Exception as e:
        raise click.ClickException(f"Failed to create engine: {e}") from e

    async def _run():
        return await session.remove_document(doc_id)

    try:
        removed = asyncio.run(_run())
    except Exception as e:
        raise click.ClickException(f"Failed to remove document: {e}") from e

    if removed:
        click.echo(f"Removed document: {doc_id}")
    else:
        raise click.ClickException(f"Document not found: {doc_id}")
