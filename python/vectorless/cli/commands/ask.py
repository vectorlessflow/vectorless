"""ask command — interactive REPL for multi-turn queries."""

from typing import Optional

import click


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
    raise NotImplementedError


def _handle_repl_command(line: str) -> bool:
    """Handle a built-in REPL command (prefixed with .).

    Args:
        line: Raw input line.

    Returns:
        True if the command was handled, False if it's a query.
    """
    raise NotImplementedError


def _print_welcome() -> None:
    """Print REPL welcome message with available commands."""
    raise NotImplementedError
