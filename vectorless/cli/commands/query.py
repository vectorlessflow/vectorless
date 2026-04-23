"""query command — single query (maps to engine.query)."""

import asyncio
import os
from typing import Optional

import click

from vectorless.cli.workspace import get_workspace_path
from vectorless.cli.output import OutputFormat, format_query_result


def _create_engine(workspace_dir: str):
    """Create an Engine from workspace config."""
    from vectorless.engine import Engine

    config_path = os.path.join(workspace_dir, "config.toml")
    if os.path.exists(config_path):
        return Engine.from_config_file(config_path)
    return Engine.from_env()


def query_cmd(
    question: str,
    *,
    doc_ids: tuple[str, ...] = (),
    fmt: str = "text",
    verbose: bool = False,
    timeout_secs: Optional[int] = None,
) -> None:
    """Execute a single query against indexed documents.

    Args:
        question: Natural-language question.
        doc_ids: Limit to specific document IDs.
        fmt: Output format — "text" or "json".
        verbose: Show Agent navigation steps.
        timeout_secs: Per-operation timeout in seconds.
    """
    workspace = get_workspace_path()

    try:
        session = _create_engine(workspace)
    except Exception as e:
        raise click.ClickException(f"Failed to create engine: {e}") from e

    async def _run():
        return await session.ask(
            question,
            doc_ids=list(doc_ids) if doc_ids else None,
            timeout_secs=timeout_secs,
        )

    try:
        result = asyncio.run(_run())
    except Exception as e:
        raise click.ClickException(f"Query failed: {e}") from e

    output_fmt = OutputFormat.JSON if fmt == "json" else OutputFormat.TEXT
    output = format_query_result(result, fmt=output_fmt, verbose=verbose)
    click.echo(output)

    # Show metrics in verbose mode
    if verbose:
        m = result.metrics
        click.echo(
            f"\nMetrics: "
            f"LLM calls={m.llm_calls}, "
            f"rounds={m.rounds_used}, "
            f"nodes_visited={m.nodes_visited}, "
            f"evidence_chars={m.evidence_chars}"
        )
