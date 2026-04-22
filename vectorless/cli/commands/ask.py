"""ask command — interactive REPL for multi-turn queries."""

import asyncio
import os
from typing import Optional

import click

from vectorless.cli.workspace import get_workspace_path
from vectorless.cli.output import OutputFormat, format_query_result


def _create_session(workspace_dir: str):
    """Create a Session from workspace config."""
    from vectorless.session import Session

    config_path = os.path.join(workspace_dir, "config.toml")
    if os.path.exists(config_path):
        return Session.from_config_file(config_path)
    return Session.from_env()


# Module-level mutable state for the REPL
_current_doc_id: Optional[str] = None
_verbose: bool = False
_total_llm_calls: int = 0
_total_queries: int = 0


def _print_welcome() -> None:
    """Print REPL welcome message with available commands."""
    click.echo("Vectorless Interactive REPL")
    click.echo("Type a question to query your documents.")
    click.echo("")
    click.echo("Dot-commands:")
    click.echo("  .help       Show available commands")
    click.echo("  .tree       Display current document tree")
    click.echo("  .stats      Show session statistics (LLM calls, tokens, cost)")
    click.echo("  .nav-log    Show navigation log for current conversation")
    click.echo("  .doc <id>   Switch query target document")
    click.echo("  .doc        Show current target document")
    click.echo("  .verbose    Toggle verbose mode")
    click.echo("  .quit       Exit REPL")
    click.echo("")


def _handle_repl_command(
    line: str,
    session,
    workspace: str,
) -> Optional[bool]:
    """Handle a built-in REPL command (prefixed with .).

    Args:
        line: Raw input line.
        session: Session instance.
        workspace: Workspace path.

    Returns:
        True if the command was handled (should not be treated as query).
        False if it's a query.
        None if the REPL should exit.
    """
    global _current_doc_id, _verbose, _total_llm_calls, _total_queries

    parts = line.strip().split(maxsplit=1)
    cmd = parts[0].lower()
    arg = parts[1] if len(parts) > 1 else None

    if cmd == ".quit":
        return None
    elif cmd == ".help":
        _print_welcome()
        return True
    elif cmd == ".verbose":
        _verbose = not _verbose
        state = "on" if _verbose else "off"
        click.echo(f"Verbose mode: {state}")
        return True
    elif cmd == ".doc":
        if arg:
            _current_doc_id = arg
            click.echo(f"Now targeting document: {_current_doc_id}")
        else:
            if _current_doc_id:
                click.echo(f"Current document: {_current_doc_id}")
            else:
                click.echo("No document target set (querying all documents)")
        return True
    elif cmd == ".stats":
        click.echo(f"Session statistics:")
        click.echo(f"  Queries: {_total_queries}")
        click.echo(f"  LLM calls (from query metrics): {_total_llm_calls}")

        try:
            report = session.metrics_report()
            if report:
                click.echo(f"  Engine metrics: {report}")
        except Exception:
            pass
        return True
    elif cmd == ".tree":
        if _current_doc_id:
            click.echo(f"Tree visualization for {_current_doc_id}:")
            click.echo("  (Use 'vectorless tree' command for full tree display)")
        else:
            click.echo("No document selected. Use .doc <id> to select one.")
        return True
    elif cmd == ".nav-log":
        click.echo("Navigation log is shown when verbose mode is on (.verbose)")
        return True
    else:
        click.echo(f"Unknown command: {cmd}. Type .help for available commands.")
        return True


def ask_cmd(*, doc_id: Optional[str] = None, verbose: bool = False) -> None:
    """Start an interactive query REPL.

    Args:
        doc_id: Limit to a specific document.
        verbose: Show Agent navigation steps.

    Uses:
        Engine.query() in a loop with user input.
        Maintains conversation context across turns.

    Built-in commands (prefixed with .):
        .help       Show available commands
        .tree       Display current document tree
        .stats      Show session statistics (LLM calls, tokens, cost)
        .nav-log    Show navigation log for current conversation
        .doc <id>   Switch query target document
        .doc        Show current target document
        .verbose    Toggle verbose mode
        .quit       Exit REPL
    """
    global _current_doc_id, _verbose, _total_llm_calls, _total_queries

    _current_doc_id = doc_id
    _verbose = verbose
    _total_llm_calls = 0
    _total_queries = 0

    workspace = get_workspace_path()

    try:
        session = _create_session(workspace)
    except Exception as e:
        raise click.ClickException(f"Failed to create session: {e}") from e

    _print_welcome()

    while True:
        try:
            line = input(">>> ").strip()
        except (EOFError, KeyboardInterrupt):
            click.echo("\nGoodbye!")
            break

        if not line:
            continue

        # Handle dot-commands
        if line.startswith("."):
            result = _handle_repl_command(line, session, workspace)
            if result is None:
                click.echo("Goodbye!")
                break
            continue

        # Treat as a query
        _total_queries += 1

        try:
            # Build query arguments
            doc_ids = [_current_doc_id] if _current_doc_id else None

            async def _run():
                return await session.ask(
                    line,
                    doc_ids=doc_ids,
                )

            response = asyncio.run(_run())

            # Accumulate metrics
            for item in response.items:
                if item.metrics:
                    _total_llm_calls += item.metrics.llm_calls

            output = format_query_result(
                response, fmt=OutputFormat.TEXT, verbose=_verbose
            )
            click.echo(output)

        except Exception as e:
            click.echo(f"Error: {e}")
