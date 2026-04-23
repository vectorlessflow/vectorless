"""LLM-dependent agent tools: compare, trace, summarize.

These tools combine Rust compute primitives with LLM cognitive operations.
The Worker fetches content via Rust primitives, then these functions use LLM
for analysis. This keeps the "thick boundary" — Rust does compute, Python does
LLM-dependent strategy.
"""

from __future__ import annotations

import logging

from vectorless.llm_client import LLMClient

logger = logging.getLogger(__name__)

_MAX_COMPARE_CHARS = 2000
_MAX_TRACE_CHARS = 3000
_MAX_SUMMARIZE_CHARS = 3000


def _truncate(content: str, max_len: int) -> str:
    if len(content) > max_len:
        return content[:max_len] + "..."
    return content


async def compare_nodes(
    title_a: str,
    content_a: str,
    title_b: str,
    content_b: str,
    llm: LLMClient,
    *,
    query: str = "",
) -> str:
    """Compare two document sections using LLM."""
    content_a = _truncate(content_a, _MAX_COMPARE_CHARS)
    content_b = _truncate(content_b, _MAX_COMPARE_CHARS)

    query_ctx = f"\nContext: the user asked: {query}" if query else ""

    system = (
        "You are a document analysis assistant. Compare the two sections below. "
        "Identify key similarities and differences. Be specific and concise."
    )
    user = (
        f"Section A: [{title_a}]\n{content_a}\n\n"
        f"Section B: [{title_b}]\n{content_b}"
        f"{query_ctx}\n\n"
        f"Comparison:"
    )

    try:
        return (await llm.complete(system, user)).strip()
    except Exception as e:
        logger.warning("Compare LLM call failed: %s", e)
        return f"Comparison failed: {e}"


async def trace_reasoning(
    title: str,
    content: str,
    related_context: str,
    llm: LLMClient,
    *,
    query: str = "",
) -> str:
    """Trace reasoning chain from a document section using LLM."""
    content = _truncate(content, _MAX_TRACE_CHARS)

    query_ctx = f"\nThe user asked: {query}" if query else ""

    system = (
        "You are a reasoning trace analyst. Given a document section, trace the logical "
        "argument chain: identify premises, conclusions, supporting evidence, and logical "
        "connections. If related sections are listed, note how they connect to the argument."
    )
    user = (
        f"Section: [{title}]\n{content}"
        f"{related_context}"
        f"{query_ctx}\n\n"
        f"Reasoning trace:"
    )

    try:
        return (await llm.complete(system, user)).strip()
    except Exception as e:
        logger.warning("Trace LLM call failed: %s", e)
        return f"Trace failed: {e}"


async def summarize_section(
    title: str,
    content: str,
    llm: LLMClient,
    *,
    query: str = "",
) -> str:
    """Generate a dynamic LLM summary of a document section."""
    content = _truncate(content, _MAX_SUMMARIZE_CHARS)

    query_ctx = f"\nFocus the summary for the question: {query}" if query else ""

    system = (
        "You are a document summarizer. Provide a concise summary of the section below. "
        "Highlight key facts, conclusions, and data points."
    )
    user = (
        f"Section: [{title}]\n{content}"
        f"{query_ctx}\n\n"
        f"Summary:"
    )

    try:
        return (await llm.complete(system, user)).strip()
    except Exception as e:
        logger.warning("Summarize LLM call failed: %s", e)
        return f"Summarize failed: {e}"
