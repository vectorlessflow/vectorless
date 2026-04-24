"""Command execution registry — each command is a standalone async function.

To add a new command:
1. Write an async handler: ``async def handle_<cmd>(command, doc, state, query, llm) -> Step``
2. Register it: ``_REGISTRY["<cmd>"] = handle_<cmd>``

The main Worker loop looks up the handler in ``_REGISTRY`` and calls it.
No modification to the main loop is needed.
"""

from __future__ import annotations

import logging
import re
from dataclasses import dataclass
from typing import Any, Callable, Awaitable

from vectorless.ask.protocols import NavigableDocument
from vectorless.ask.types import Evidence, WorkerState
from vectorless.llm_client import LLMClient
from vectorless.ask.tools import compare_nodes, summarize_section, trace_reasoning
from vectorless.ask.prompts import check_sufficiency, parse_sufficiency_response

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Step result — shared across all commands
# ---------------------------------------------------------------------------

@dataclass
class Step:
    """Result of a single command execution."""
    kind: str  # continue, done, force_done
    reason: str = ""


# ---------------------------------------------------------------------------
# Shared helpers
# ---------------------------------------------------------------------------

async def _resolve_target(doc: NavigableDocument, target: str, state: WorkerState) -> str | None:
    """Resolve a command target (node ID, child title, or empty for current) to a node ID."""
    if not target or target == ".":
        return await doc.current_id()
    if re.match(r"^n\d+$", target):
        return target
    children = await doc.ls()
    for child in children:
        if child.title.lower() == target.lower():
            return child.id
    for child in children:
        if target.lower() in child.title.lower():
            return child.id
    return None


async def _visited_titles(state: WorkerState, doc: NavigableDocument) -> str:
    """Format visited node titles for prompt context."""
    titles = []
    for node_id in state.visited:
        try:
            title = await doc.node_title(node_id)
            if title:
                titles.append(title)
        except Exception:
            pass
    return ", ".join(titles) if titles else "(none)"


# Type alias for the handler signature
CommandHandler = Callable[..., Awaitable[Step]]


# ---------------------------------------------------------------------------
# Individual command handlers
# ---------------------------------------------------------------------------

async def handle_ls(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    children = await doc.ls()
    if not children:
        state.last_feedback = "(no navigation data)"
    else:
        lines = []
        for i, child in enumerate(children, 1):
            hints = getattr(child, "question_hints", [])
            tags = getattr(child, "topic_tags", [])
            annotations = []
            if hints:
                for h in hints[:2]:
                    annotations.append(f'question "{h}"')
            if tags:
                for t in tags[:2]:
                    annotations.append(f'topic "{t}"')
            ann_str = f", {', '.join(annotations)}" if annotations else ""
            lines.append(
                f"[{i}] {child.title} — "
                f"(depth {child.depth}, {child.leaf_count} leaves{ann_str})"
            )
        state.last_feedback = "\n".join(lines)
    state.visited.add(await doc.current_id())
    return Step(kind="continue")


async def handle_cd(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    target = command.target
    if not target:
        state.last_feedback = "Usage: cd <name>"
        return Step(kind="continue")

    # Try cd by node id first (if target looks like n42)
    if re.match(r"^n\d+$", target):
        try:
            await doc.cd(target)
            title = await doc.node_title(target)
            state.breadcrumb.append(title)
            current = await doc.current_id()
            state.visited.add(current)
            state.last_feedback = f"Entered '{title}'"
            return Step(kind="continue")
        except Exception:
            pass

    # Try cd_by_title
    try:
        await doc.cd_by_title(target)
        current = await doc.current_id()
        title = await doc.node_title(current)
        state.breadcrumb.append(title)
        state.visited.add(current)
        state.last_feedback = f"Entered '{title}'"
        return Step(kind="continue")
    except Exception:
        state.last_feedback = f"Node '{target}' not found. Use ls to list children."
        return Step(kind="continue")


async def handle_cd_up(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    try:
        await doc.cd_up()
        if len(state.breadcrumb) > 1:
            state.breadcrumb.pop()
        state.last_feedback = f"Current position: /{state.path_str()}"
    except Exception as e:
        state.last_feedback = f"Cannot go up: {e}"
    return Step(kind="continue")


async def handle_cat(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    target = command.target
    node_id = None

    if target == "." or target == "":
        node_id = await doc.current_id()
    elif re.match(r"^n\d+$", target):
        node_id = target
    else:
        # Try to find by title among children
        children = await doc.ls()
        for child in children:
            if child.title.lower() == target.lower():
                node_id = child.id
                break
            if target.lower() in child.title.lower():
                node_id = child.id
                break
        if node_id is None:
            # Try find
            results = await doc.find(target)
            if results:
                node_id = results[0].node_id

    if node_id is None:
        state.last_feedback = f"Node '{target}' not found."
        return Step(kind="continue")

    if node_id in state.collected_nodes:
        state.last_feedback = f"Already collected evidence from '{target}'. Use done if sufficient."
        return Step(kind="continue")

    try:
        content = await doc.cat(node_id)
        title = await doc.node_title(node_id)
        pwd = await doc.pwd()

        evidence = Evidence(
            source_path=pwd,
            node_title=title,
            content=content,
        )
        state.evidence.append(evidence)
        state.collected_nodes.add(node_id)
        state.visited.add(node_id)

        preview = content[:500] + "..." if len(content) > 500 else content
        state.last_feedback = f"[{title}] collected as evidence:\n{preview}"
        return Step(kind="continue")
    except Exception as e:
        state.last_feedback = f"Error reading node: {e}"
        return Step(kind="continue")


async def handle_find(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    keyword = command.target
    if not keyword:
        state.last_feedback = "Usage: find <keyword>"
        return Step(kind="continue")

    try:
        results = await doc.find(keyword)
    except Exception:
        results = []

    if not results:
        # Fallback: try keyword_entries for reasoning index hits
        try:
            entries = await doc.keyword_entries(keyword)
            if entries:
                lines = [f"Results for '{keyword}':"]
                for entry in entries:
                    title = await doc.node_title(entry.node_id)
                    lines.append(
                        f"  - {title} (depth {entry.depth}, weight {entry.weight:.2f})"
                    )
                state.last_feedback = "\n".join(lines)
                return Step(kind="continue")
        except Exception:
            pass
        state.last_feedback = f"No results for '{keyword}'."
        return Step(kind="continue")

    lines = [f"Results for '{keyword}':"]
    for r in results[:10]:
        lines.append(f"  - {r.title} (depth {r.depth}, {r.leaf_count} leaves)")
    state.last_feedback = "\n".join(lines)
    return Step(kind="continue")


async def handle_findtree(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    pattern = command.target
    if not pattern:
        state.last_feedback = "Usage: findtree <pattern>"
        return Step(kind="continue")

    try:
        results = await doc.find(pattern)
    except Exception:
        results = []

    if not results:
        state.last_feedback = f"No nodes matching '{pattern}' in titles."
        return Step(kind="continue")

    lines = [f"Nodes matching '{pattern}':"]
    for r in results[:15]:
        lines.append(f"  - {r.title} (depth {r.depth})")
    state.last_feedback = "\n".join(lines)
    return Step(kind="continue")


async def handle_grep(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    pattern = command.target
    if not pattern:
        state.last_feedback = "Usage: grep <pattern>"
        return Step(kind="continue")
    try:
        matches = await doc.grep(pattern)
    except Exception as e:
        state.last_feedback = f"grep error: {e}"
        return Step(kind="continue")

    if not matches:
        state.last_feedback = f"No matches for /{pattern}/."
        return Step(kind="continue")

    lines = [f"Matches for /{pattern}/:"]
    for m in matches[:15]:
        lines.append(f"  - {m.title} (line {m.line_number}): {m.snippet[:100]}")
    state.last_feedback = "\n".join(lines)
    return Step(kind="continue")


async def handle_head(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    target = command.target
    n = command.lines
    node_id = None

    if re.match(r"^n\d+$", target):
        node_id = target
    else:
        children = await doc.ls()
        for child in children:
            if child.title.lower() == target.lower():
                node_id = child.id
                break

    if node_id is None:
        state.last_feedback = f"Node '{target}' not found."
        return Step(kind="continue")

    try:
        content = await doc.head(node_id, n)
        state.last_feedback = content
    except Exception as e:
        state.last_feedback = f"head error: {e}"
    return Step(kind="continue")


async def handle_wc(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    target = command.target
    node_id = None

    if not target:
        node_id = await doc.current_id()
    elif re.match(r"^n\d+$", target):
        node_id = target
    else:
        children = await doc.ls()
        for child in children:
            if child.title.lower() == target.lower():
                node_id = child.id
                break

    if node_id is None:
        state.last_feedback = f"Node '{target}' not found."
        return Step(kind="continue")

    try:
        wc = await doc.wc(node_id)
        state.last_feedback = f"{wc.lines} lines, {wc.words} words, {wc.chars} chars"
    except Exception as e:
        state.last_feedback = f"wc error: {e}"
    return Step(kind="continue")


async def handle_pwd(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    try:
        pwd = await doc.pwd()
        state.last_feedback = f"/{pwd}"
    except Exception as e:
        state.last_feedback = f"pwd error: {e}"
    return Step(kind="continue")


async def handle_back(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    try:
        await doc.back()
        pwd = await doc.pwd()
        state.breadcrumb = [p for p in pwd.split("/") if p]
        state.last_feedback = f"Current position: /{state.path_str()}"
    except Exception as e:
        state.last_feedback = f"Cannot go back: {e}"
    return Step(kind="continue")


async def handle_toc(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    try:
        if command.lines > 0:
            entries = await doc.toc(command.lines)
        else:
            entries = await doc.toc()
        if not entries:
            state.last_feedback = "(empty table of contents)"
        else:
            lines = ["Table of contents:"]
            for entry in entries:
                indent = "  " * entry.depth
                children = f" ({entry.child_count} children)" if entry.child_count > 0 else ""
                lines.append(f"{indent}- {entry.title}{children}")
            state.last_feedback = "\n".join(lines)
    except Exception as e:
        state.last_feedback = f"toc error: {e}"
    return Step(kind="continue")


async def handle_stats(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    node_id = await _resolve_target(doc, command.target, state)
    if node_id is None:
        state.last_feedback = f"Node '{command.target}' not found."
        return Step(kind="continue")
    try:
        s = await doc.stats(node_id)
        leaf = " (leaf)" if s.is_leaf else ""
        state.last_feedback = (
            f"[{s.title}] depth={s.depth}, children={s.child_count}, "
            f"leaves={s.leaf_count}, chars={s.char_count}, words={s.word_count}{leaf}"
        )
    except Exception as e:
        state.last_feedback = f"stats error: {e}"
    return Step(kind="continue")


async def handle_grep_node(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    target = command.target
    pattern = command.target_b
    if not target or not pattern:
        state.last_feedback = "Usage: grep_node <node> <pattern>"
        return Step(kind="continue")
    node_id = await _resolve_target(doc, target, state)
    if node_id is None:
        state.last_feedback = f"Node '{target}' not found."
        return Step(kind="continue")
    try:
        matches = await doc.grep_node(node_id, pattern)
        if not matches:
            state.last_feedback = f"No matches for /{pattern}/ in this node."
        else:
            lines = [f"Matches for /{pattern}/:"]
            for m in matches[:15]:
                lines.append(f"  - line {m.line_number}: {m.snippet[:100]}")
            state.last_feedback = "\n".join(lines)
    except Exception as e:
        state.last_feedback = f"grep_node error: {e}"
    return Step(kind="continue")


async def handle_similar(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    node_id = await _resolve_target(doc, command.target, state)
    if node_id is None:
        state.last_feedback = f"Node '{command.target}' not found."
        return Step(kind="continue")
    try:
        results = await doc.similar(node_id)
        if not results:
            state.last_feedback = "No similar nodes found."
        else:
            lines = ["Similar nodes:"]
            for r in results[:10]:
                kw = ", ".join(r.shared_keywords[:3])
                lines.append(f"  - {r.title} (relevance: {r.relevance:.2f}, shared: {kw})")
            state.last_feedback = "\n".join(lines)
    except Exception as e:
        state.last_feedback = f"similar error: {e}"
    return Step(kind="continue")


async def handle_section_overview(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    node_id = await _resolve_target(doc, command.target, state)
    if node_id is None:
        state.last_feedback = f"Node '{command.target}' not found."
        return Step(kind="continue")
    try:
        overview = await doc.section_overview(node_id)
        state.last_feedback = overview if overview else "(no overview available)"
    except Exception as e:
        state.last_feedback = f"overview error: {e}"
    return Step(kind="continue")


async def handle_compare(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    target_a = command.target
    target_b = command.target_b
    if not target_a or not target_b:
        state.last_feedback = "Usage: compare <node_a> <node_b>"
        return Step(kind="continue")
    node_a = await _resolve_target(doc, target_a, state)
    node_b = await _resolve_target(doc, target_b, state)
    if node_a is None:
        state.last_feedback = f"Node '{target_a}' not found."
        return Step(kind="continue")
    if node_b is None:
        state.last_feedback = f"Node '{target_b}' not found."
        return Step(kind="continue")
    try:
        content_a = await doc.cat(node_a)
        title_a = await doc.node_title(node_a)
        content_b = await doc.cat(node_b)
        title_b = await doc.node_title(node_b)
        if node_a not in state.collected_nodes:
            pwd_a = await doc.pwd()
            state.evidence.append(Evidence(
                source_path=pwd_a, node_title=title_a, content=content_a,
            ))
            state.collected_nodes.add(node_a)
        if node_b not in state.collected_nodes:
            pwd_b = await doc.pwd()
            state.evidence.append(Evidence(
                source_path=pwd_b, node_title=title_b, content=content_b,
            ))
            state.collected_nodes.add(node_b)
        result = await compare_nodes(title_a, content_a, title_b, content_b, llm, query=query)
        state.llm_calls += 1
        state.last_feedback = f"Comparison of [{title_a}] vs [{title_b}]:\n{result}"
    except Exception as e:
        state.last_feedback = f"compare error: {e}"
    return Step(kind="continue")


async def handle_trace(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    node_id = await _resolve_target(doc, command.target, state)
    if node_id is None:
        state.last_feedback = f"Node '{command.target}' not found."
        return Step(kind="continue")
    try:
        content = await doc.cat(node_id)
        title = await doc.node_title(node_id)
        if node_id not in state.collected_nodes:
            pwd = await doc.pwd()
            state.evidence.append(Evidence(
                source_path=pwd, node_title=title, content=content,
            ))
            state.collected_nodes.add(node_id)
        related_context = ""
        try:
            similar = await doc.similar(node_id)
            if similar:
                related_lines = [f"  - {s.title} (relevance: {s.relevance:.2f})" for s in similar[:5]]
                related_context = "\nRelated sections:\n" + "\n".join(related_lines)
        except Exception:
            pass
        result = await trace_reasoning(title, content, related_context, llm, query=query)
        state.llm_calls += 1
        state.last_feedback = f"Reasoning trace for [{title}]:\n{result}"
    except Exception as e:
        state.last_feedback = f"trace error: {e}"
    return Step(kind="continue")


async def handle_summarize(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    node_id = await _resolve_target(doc, command.target, state)
    if node_id is None:
        state.last_feedback = f"Node '{command.target}' not found."
        return Step(kind="continue")
    try:
        content = await doc.cat(node_id)
        title = await doc.node_title(node_id)
        if node_id not in state.collected_nodes:
            pwd = await doc.pwd()
            state.evidence.append(Evidence(
                source_path=pwd, node_title=title, content=content,
            ))
            state.collected_nodes.add(node_id)
        result = await summarize_section(title, content, llm, query=query)
        state.llm_calls += 1
        state.last_feedback = f"Summary of [{title}]:\n{result}"
    except Exception as e:
        state.last_feedback = f"summarize error: {e}"
    return Step(kind="continue")


async def handle_siblings(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    node_id = await _resolve_target(doc, command.target, state)
    if node_id is None:
        state.last_feedback = f"Node '{command.target}' not found."
        return Step(kind="continue")
    try:
        siblings = await doc.siblings(node_id)
        if not siblings:
            state.last_feedback = "(no sibling nodes)"
        else:
            lines = ["Sibling nodes:"]
            for s in siblings:
                lines.append(
                    f"  - {s.title} (depth {s.depth}, {s.leaf_count} leaves)"
                )
            state.last_feedback = "\n".join(lines)
    except Exception as e:
        state.last_feedback = f"siblings error: {e}"
    return Step(kind="continue")


async def handle_ancestors(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    node_id = await _resolve_target(doc, command.target, state)
    if node_id is None:
        state.last_feedback = f"Node '{command.target}' not found."
        return Step(kind="continue")
    try:
        ancestors = await doc.ancestors(node_id)
        if not ancestors:
            state.last_feedback = "(at root, no ancestors)"
        else:
            lines = ["Path from root:"]
            for a in ancestors:
                lines.append(
                    f"  {'  ' * a.depth}\u2192 {a.title} (depth {a.depth}, {a.child_count} children)"
                )
            state.last_feedback = "\n".join(lines)
    except Exception as e:
        state.last_feedback = f"ancestors error: {e}"
    return Step(kind="continue")


async def handle_doc_card(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    try:
        card = await doc.doc_card()
        if card is None:
            state.last_feedback = "(no document card available)"
        else:
            lines = [
                f"Document: {card.title}",
                f"Overview: {card.overview}",
                f"Total leaves: {card.total_leaves}",
            ]
            if card.question_hints:
                lines.append(f"Can answer: {', '.join(card.question_hints[:5])}")
            if card.topic_tags:
                lines.append(f"Topics: {', '.join(card.topic_tags[:5])}")
            if card.sections:
                lines.append("Top-level sections:")
                for s in card.sections:
                    lines.append(f"  - {s.title}: {s.description} ({s.leaf_count} leaves)")
            state.last_feedback = "\n".join(lines)
    except Exception as e:
        state.last_feedback = f"doc_card error: {e}"
    return Step(kind="continue")


async def handle_concepts(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    try:
        concepts = await doc.concepts()
        if not concepts:
            state.last_feedback = "(no concepts extracted)"
        else:
            lines = ["Key concepts:"]
            for c in concepts:
                sections = ", ".join(c.sections[:3])
                lines.append(f"  - {c.name}: {c.summary} (in: {sections})")
            state.last_feedback = "\n".join(lines)
    except Exception as e:
        state.last_feedback = f"concepts error: {e}"
    return Step(kind="continue")


async def handle_find_section(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    title = command.target
    if not title:
        state.last_feedback = "Usage: find_section <title>"
        return Step(kind="continue")
    try:
        result = await doc.find_section(title)
        if result is None:
            state.last_feedback = f"No section with title '{title}'."
        else:
            state.last_feedback = (
                f"Found: {result.title} (id={result.node_id}, "
                f"depth {result.depth}, {result.leaf_count} leaves)"
            )
    except Exception as e:
        state.last_feedback = f"find_section error: {e}"
    return Step(kind="continue")


async def handle_check(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    evidence_text = state.evidence_for_check()
    system, user = check_sufficiency(query, evidence_text)

    try:
        response = await llm.complete(system, user)
    except Exception as e:
        logger.warning("Check LLM call failed: %s", e)
        state.last_feedback = "Could not evaluate sufficiency."
        return Step(kind="continue")

    state.llm_calls += 1
    state.check_count += 1
    sufficient = parse_sufficiency_response(response)

    if sufficient:
        state.last_feedback = "Evidence is sufficient. Use done to finish."
        return Step(kind="done")
    else:
        # Extract missing info
        reason = response.strip()
        for prefix in ("INSUFFICIENT", "Insufficient"):
            if reason.startswith(prefix):
                reason = reason[len(prefix):]
                break
        reason = reason.lstrip("-: ")
        if reason:
            state.missing_info = reason
        state.last_feedback = f"Evidence not yet sufficient: {response.strip()}"
        return Step(kind="continue")


async def handle_done(
    command: Any, doc: NavigableDocument, state: WorkerState, query: str, llm: LLMClient,
) -> Step:
    state.last_feedback = "Navigation complete."
    return Step(kind="done")


# ---------------------------------------------------------------------------
# Registry — maps command kind string to handler function
# ---------------------------------------------------------------------------

_REGISTRY: dict[str, CommandHandler] = {
    "ls": handle_ls,
    "cd": handle_cd,
    "cd_up": handle_cd_up,
    "cat": handle_cat,
    "find": handle_find,
    "findtree": handle_findtree,
    "grep": handle_grep,
    "head": handle_head,
    "wc": handle_wc,
    "pwd": handle_pwd,
    "back": handle_back,
    "toc": handle_toc,
    "stats": handle_stats,
    "grep_node": handle_grep_node,
    "similar": handle_similar,
    "section_overview": handle_section_overview,
    "compare": handle_compare,
    "trace": handle_trace,
    "summarize": handle_summarize,
    "siblings": handle_siblings,
    "ancestors": handle_ancestors,
    "doc_card": handle_doc_card,
    "concepts": handle_concepts,
    "find_section": handle_find_section,
    "check": handle_check,
    "done": handle_done,
}


async def execute_command(
    command: Any,
    doc: NavigableDocument,
    state: WorkerState,
    query: str,
    llm: LLMClient,
) -> Step:
    """Execute a parsed command via the registry.

    Looks up ``command.kind`` in ``_REGISTRY`` and dispatches to the handler.
    Falls back to ``handle_ls`` for unknown commands.
    """
    handler = _REGISTRY.get(command.kind, handle_ls)
    return await handler(command, doc, state, query, llm)
