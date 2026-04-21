"""add command — index documents (maps to engine.index)."""

import asyncio
import os
from pathlib import Path
from typing import Optional

import click

from vectorless.cli.workspace import get_workspace_path
from vectorless.cli.output import format_json


def _create_session(workspace_dir: str):
    """Create a Session from workspace config.

    Args:
        workspace_dir: Path to .vectorless/ directory.

    Returns:
        Configured Session instance.
    """
    from vectorless.session import Session

    config_path = os.path.join(workspace_dir, "config.toml")
    if os.path.exists(config_path):
        return Session.from_config_file(config_path)
    return Session.from_env()


def add_cmd(
    path: str,
    *,
    recursive: bool = False,
    fmt: Optional[str] = None,
    force: bool = False,
    jobs: int = 1,
    verbose: bool = False,
) -> None:
    """Index a document or directory.

    Args:
        path: File or directory path.
        recursive: Index directory recursively.
        fmt: Force format ("markdown" | "pdf" | None for auto-detect).
        force: Force re-index existing documents.
        jobs: Number of parallel indexing jobs.
        verbose: Show detailed progress.

    Uses:
        Engine.index(IndexContext)
        IndexContext.from_path / from_paths / from_dir
        IndexOptions(mode="force" if force else "default")
    """
    workspace = get_workspace_path()

    try:
        session = _create_session(workspace)
    except Exception as e:
        raise click.ClickException(f"Failed to create session: {e}") from e

    target = Path(path).resolve()
    format_hint = fmt or "markdown"

    async def _run():
        if target.is_dir():
            if recursive:
                # Collect all matching files in directory
                extensions = {".md", ".pdf", ".markdown"}
                file_paths = [
                    str(f)
                    for f in target.rglob("*")
                    if f.suffix.lower() in extensions and f.is_file()
                ]
            else:
                extensions = {".md", ".pdf", ".markdown"}
                file_paths = [
                    str(f)
                    for f in target.iterdir()
                    if f.suffix.lower() in extensions and f.is_file()
                ]

            if not file_paths:
                raise click.ClickException(
                    f"No supported documents found in {target}"
                )

            if verbose:
                click.echo(f"Found {len(file_paths)} document(s) to index")

            results = await session.index_batch(
                file_paths, mode="force" if force else "default", jobs=jobs
            )

            succeeded = [r for r in results if not r.has_failures()]
            failed = [r for r in results if r.has_failures()]

            click.echo(f"Indexed {len(succeeded)}/{len(results)} document(s) successfully")
            if failed:
                click.echo(f"Failed: {len(failed)} document(s)")
                for f_result in failed:
                    for item in f_result.failed:
                        click.echo(f"  {item.source}: {item.error}")

            if verbose:
                for r in succeeded:
                    for item in r.items:
                        click.echo(f"  {item.name} ({item.doc_id})")
        else:
            result = await session.index(
                path=str(target),
                format=format_hint,
                mode="force" if force else "default",
            )

            if result.doc_id:
                click.echo(f"Indexed: {result.doc_id}")
            else:
                # Batch result from single file
                for item in result.items:
                    click.echo(f"Indexed: {item.name} ({item.doc_id})")

            if result.has_failures():
                for item in result.failed:
                    click.echo(f"Failed: {item.source}: {item.error}")

            if verbose and result.items:
                for item in result.items:
                    if item.metrics:
                        m = item.metrics
                        click.echo(
                            f"  Nodes: {m.nodes_processed}, "
                            f"Summaries: {m.summaries_generated}, "
                            f"LLM calls: {m.llm_calls}, "
                            f"Time: {m.total_time_ms}ms"
                        )

    try:
        asyncio.run(_run())
    except click.ClickException:
        raise
    except Exception as e:
        raise click.ClickException(f"Indexing failed: {e}") from e
