"""Output formatting — text, json, table."""

from __future__ import annotations

import json
from enum import Enum
from typing import Any, Dict, List, Optional


class OutputFormat(Enum):
    TEXT = "text"
    JSON = "json"
    TABLE = "table"


def format_result(data: Any, fmt: OutputFormat) -> str:
    """Format a result dict for terminal output."""
    if fmt == OutputFormat.JSON:
        return format_json(data)
    return format_text(data)


def format_text(data: Any) -> str:
    """Format data as readable text."""
    if isinstance(data, dict):
        lines = []
        for key, value in data.items():
            lines.append(f"  {key}: {value}")
        return "\n".join(lines)
    return str(data)


def format_documents_table(documents: List[Any]) -> str:
    """Format a list of documents as a table.

    Columns: Doc ID | Title | Format | Pages | Source

    Uses rich if available, plain text otherwise.
    """
    if not documents:
        return "No documents indexed."

    try:
        from rich.console import Console
        from rich.table import Table

        table = Table(title="Indexed Documents")
        table.add_column("Doc ID", style="cyan", no_wrap=True, max_width=12)
        table.add_column("Title", style="white")
        table.add_column("Format", style="green")
        table.add_column("Pages", style="yellow", justify="right")
        table.add_column("Source", style="dim")

        for doc in documents:
            doc_id = doc.id if hasattr(doc, "id") else str(doc.get("id", ""))
            name = doc.name if hasattr(doc, "name") else str(doc.get("name", ""))
            fmt = doc.format if hasattr(doc, "format") else str(doc.get("format", ""))
            pages = doc.page_count if hasattr(doc, "page_count") else doc.get("page_count")
            source = (
                doc.source_path if hasattr(doc, "source_path") else doc.get("source_path")
            )
            table.add_row(
                doc_id[:12],
                name,
                fmt,
                str(pages) if pages else "-",
                str(source) if source else "-",
            )

        from io import StringIO

        buf = StringIO()
        console = Console(file=buf, force_terminal=True)
        console.print(table)
        return buf.getvalue()

    except ImportError:
        # Plain text fallback
        lines = []
        header = f"{'Doc ID':<14} {'Title':<30} {'Format':<10} {'Pages':>6} {'Source'}"
        lines.append(header)
        lines.append("-" * len(header))

        for doc in documents:
            doc_id = (doc.id if hasattr(doc, "id") else str(doc.get("id", "")))[:12]
            name = (doc.name if hasattr(doc, "name") else str(doc.get("name", "")))[:28]
            fmt = doc.format if hasattr(doc, "format") else str(doc.get("format", ""))
            pages = doc.page_count if hasattr(doc, "page_count") else doc.get("page_count")
            source = (
                doc.source_path if hasattr(doc, "source_path") else doc.get("source_path")
            )
            lines.append(
                f"{doc_id:<14} {name:<30} {fmt:<10} {str(pages or '-'):>6} {source or '-'}"
            )

        return "\n".join(lines)


def format_query_result(
    result: Any,
    fmt: OutputFormat = OutputFormat.TEXT,
    verbose: bool = False,
) -> str:
    """Format query results for output.

    Args:
        result: QueryResponse or similar with items and failed.
        fmt: Output format.
        verbose: Show evidence details.
    """
    if fmt == OutputFormat.JSON:
        if hasattr(result, "to_dict"):
            return format_json(result.to_dict())
        return format_json(result)

    lines = []
    items = result.items if hasattr(result, "items") else result.get("items", [])

    for item in items:
        content = item.content if hasattr(item, "content") else item.get("content", "")
        doc_id = item.doc_id if hasattr(item, "doc_id") else item.get("doc_id", "")
        confidence = (
            item.confidence if hasattr(item, "confidence") else item.get("confidence", 0)
        )

        lines.append(f"[{doc_id}] (confidence: {confidence:.2f})")
        lines.append(f"  {content}")

        if verbose:
            evidence = (
                item.evidence if hasattr(item, "evidence") else item.get("evidence", [])
            )
            if evidence:
                lines.append("  Evidence:")
                for ev in evidence:
                    title = ev.title if hasattr(ev, "title") else ev.get("title", "")
                    path = ev.path if hasattr(ev, "path") else ev.get("path", "")
                    lines.append(f"    - {title} ({path})")

        lines.append("")

    failed = result.failed if hasattr(result, "failed") else result.get("failed", [])
    if failed:
        lines.append("Failures:")
        for f in failed:
            source = f.source if hasattr(f, "source") else f.get("source", "")
            error = f.error if hasattr(f, "error") else f.get("error", "")
            lines.append(f"  {source}: {error}")

    return "\n".join(lines)


def format_tree(
    nodes: List[Dict],
    *,
    max_depth: Optional[int] = None,
    show_summary: bool = False,
    show_keywords: bool = False,
) -> str:
    """Format document tree as indented tree view.

    Limited implementation without Rust tree exposure.
    Displays graph structure instead.
    """
    if not nodes:
        return "No tree data available."

    lines = []
    for node in nodes:
        indent = "  " * node.get("depth", 0)
        title = node.get("title", "untitled")
        lines.append(f"{indent}├── {title}")
        if show_summary and node.get("summary"):
            lines.append(f"{indent}│   summary: {node['summary'][:80]}...")
        if show_keywords and node.get("keywords"):
            kw = ", ".join(node["keywords"][:5])
            lines.append(f"{indent}│   keywords: {kw}")

    return "\n".join(lines)


def format_navigation_steps(steps: List[Dict]) -> str:
    """Format Agent navigation steps for verbose mode."""
    if not steps:
        return ""

    lines = []
    for i, step in enumerate(steps, 1):
        action = step.get("action", "?")
        target = step.get("target", "")
        reasoning = step.get("reasoning", "")
        lines.append(f"  Step {i}: {action} {target}")
        if reasoning:
            lines.append(f"    Reason: {reasoning}")

    return "\n".join(lines)


def format_json(data: Any) -> str:
    """Format data as indented JSON."""
    if hasattr(data, "to_dict"):
        data = data.to_dict()
    return json.dumps(data, indent=2, default=str, ensure_ascii=False)
