"""Jupyter rich display integration for Vectorless results."""

from __future__ import annotations

import html as html_module
from typing import Any, List, Optional

from vectorless.ask.types import Output


class QueryResultDisplay:
    """Rich display for query results in Jupyter notebooks.

    Implements _repr_html_(), _repr_markdown_(), and _repr_json_()
    for automatic rendering.
    """

    def __init__(self, result: Output) -> None:
        self._result = result

    def _repr_html_(self) -> str:
        result = self._result
        escaped_answer = html_module.escape(result.answer[:500])
        confidence_bar = _confidence_bar(result.confidence)
        evidence_html = _evidence_list_html(result.evidence)

        return (
            f"<div style='font-family:sans-serif;'>"
            f"<div style='display:flex; justify-content:space-between; "
            f"align-items:center; margin-bottom:8px;'>"
            f"<h4>Result</h4>{confidence_bar}</div>"
            f"<p style='margin:0;'>{escaped_answer}</p>"
            f"{evidence_html}"
            f"</div>"
        )

    def _repr_markdown_(self) -> str:
        result = self._result
        lines = [
            f"## Result (confidence: {result.confidence:.2f})\n",
            f"{result.answer}\n",
        ]
        if result.evidence:
            lines.append("**Evidence:**\n")
            for ev in result.evidence:
                doc_label = f" [{ev.doc_name}]" if ev.doc_name else ""
                lines.append(f"- **{ev.node_title}** ({ev.source_path}){doc_label}")
            lines.append("")
        return "\n".join(lines)

    def _repr_json_(self) -> dict:
        result = self._result
        return {
            "answer": result.answer,
            "confidence": result.confidence,
            "evidence": [
                {
                    "title": e.node_title,
                    "path": e.source_path,
                    "content": e.content,
                    "doc_name": e.doc_name,
                }
                for e in result.evidence
            ],
            "metrics": {
                "llm_calls": result.metrics.llm_calls,
                "rounds_used": result.metrics.rounds_used,
                "nodes_visited": result.metrics.nodes_visited,
                "evidence_chars": result.metrics.evidence_chars,
            },
        }


class DocumentGraphDisplay:
    """Rich display for document relationship graphs."""

    def __init__(self, graph: Any) -> None:
        self._graph = graph

    def _repr_html_(self) -> str:
        node_count = self._graph.node_count() if self._graph else 0
        edge_count = self._graph.edge_count() if self._graph else 0
        doc_ids = self._graph.doc_ids() if self._graph else []

        rows = []
        for doc_id in doc_ids:
            node = self._graph.get_node(doc_id)
            if node:
                rows.append(
                    f"<tr><td><code>{html_module.escape(node.doc_id)}</code></td>"
                    f"<td>{html_module.escape(node.title)}</td>"
                    f"<td>{node.node_count}</td></tr>"
                )

        return (
            f"<div style='font-family:sans-serif;'>"
            f"<h4>Document Graph</h4>"
            f"<p>{node_count} nodes, {edge_count} edges</p>"
            f"<table style='border-collapse:collapse; width:100%;'>"
            f"<tr style='background:#f5f5f5;'>"
            f"<th style='padding:8px; text-align:left;'>Doc ID</th>"
            f"<th style='padding:8px; text-align:left;'>Title</th>"
            f"<th style='padding:8px; text-align:right;'>Nodes</th></tr>"
            f"{''.join(rows)}</table></div>"
        )


def _confidence_bar(confidence: float) -> str:
    """Generate an HTML confidence indicator bar."""
    pct = int(confidence * 100)
    if confidence >= 0.8:
        color = "#4caf50"
    elif confidence >= 0.5:
        color = "#ff9800"
    else:
        color = "#f44336"
    return (
        f"<div style='display:flex; align-items:center; gap:6px;'>"
        f"<span style='font-size:12px;'>{pct}%</span>"
        f"<div style='width:60px; height:6px; background:#e0e0e0; border-radius:3px;'>"
        f"<div style='width:{pct}%; height:100%; background:{color}; border-radius:3px;'></div>"
        f"</div></div>"
    )


def _evidence_list_html(evidence: list) -> str:
    """Generate HTML for evidence items."""
    if not evidence:
        return ""
    items = []
    for ev in evidence[:5]:
        items.append(
            f"<li><strong>{html_module.escape(ev.node_title)}</strong> "
            f"<code>{html_module.escape(ev.source_path)}</code></li>"
        )
    extra = f" <em>(+{len(evidence) - 5} more)</em>" if len(evidence) > 5 else ""
    return f"<ul style='margin:8px 0 0 0; font-size:0.9em;'>{''.join(items)}{extra}</ul>"
