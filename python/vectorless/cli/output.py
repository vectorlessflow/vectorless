"""Output formatting — text, json, table."""

from typing import Any, Optional
from enum import Enum


class OutputFormat(Enum):
    TEXT = "text"
    JSON = "json"
    TABLE = "table"


def format_result(data: Any, fmt: OutputFormat) -> str:
    """Format a result dict for terminal output.

    Args:
        data: Structured data to format.
        fmt: Target output format.

    Returns:
        Formatted string ready to print.
    """
    raise NotImplementedError


def format_documents_table(documents: list[dict]) -> str:
    """Format a list of documents as a table.

    Columns: Doc ID | Title | Format | Nodes | Pages | Indexed At

    Args:
        documents: List of document info dicts.

    Returns:
        Formatted table string (uses comfy-table or rich).
    """
    raise NotImplementedError


def format_tree(
    nodes: list[dict],
    *,
    max_depth: Optional[int] = None,
    show_summary: bool = False,
    show_keywords: bool = False,
) -> str:
    """Format document tree as indented tree view.

    Args:
        nodes: Flat list of tree nodes with parent references.
        max_depth: Max depth to display.
        show_summary: Include summaries.
        show_keywords: Include routing keywords.

    Returns:
        Indented tree string.
    """
    raise NotImplementedError


def format_navigation_steps(steps: list[dict]) -> str:
    """Format Agent navigation steps for verbose mode.

    Args:
        steps: List of navigation step dicts with action, target, reasoning.

    Returns:
        Step-by-step navigation log string.
    """
    raise NotImplementedError


def format_json(data: Any) -> str:
    """Format data as indented JSON."""
    raise NotImplementedError
