"""list command — list indexed documents."""

import asyncio
import os

import click

from vectorless.cli.workspace import get_workspace_path
from vectorless.cli.output import format_documents_table, format_json


def _create_session(workspace_dir: str):
    """Create a Session from workspace config."""
    from vectorless.session import Session

    config_path = os.path.join(workspace_dir, "config.toml")
    if os.path.exists(config_path):
        return Session.from_config_file(config_path)
    return Session.from_env()


def list_cmd(*, fmt: str = "table") -> None:
    """List all indexed documents in the workspace.

    Args:
        fmt: Output format — "table" or "json".

    Uses:
        Engine.list() -> List[DocumentInfo]

    Table output:
        Doc ID | Title | Format | Nodes | Pages | Indexed At
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

    if not documents:
        click.echo("No documents indexed.")
        return

    if fmt == "json":
        # Convert document objects to dicts for JSON output
        doc_dicts = []
        for doc in documents:
            doc_dict = {}
            for attr in ("id", "doc_id", "name", "format", "source_path", "page_count"):
                val = getattr(doc, attr, None)
                if val is not None:
                    doc_dict[attr] = val
            doc_dicts.append(doc_dict)
        click.echo(format_json(doc_dicts))
    else:
        click.echo(format_documents_table(documents))
