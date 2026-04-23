"""Worker agent — navigates a single document to collect evidence.

The Worker uses an LLM-driven command loop:
1. Phase 0: Initial `ls` to observe the top-level structure
2. Phase 1.5 (optional): LLM generates a navigation plan from keyword hints
3. Phase 2: Main loop — LLM picks a command → execute → record trace → repeat
"""

from __future__ import annotations

import re
import logging
from dataclasses import dataclass, field
from typing import Any

from vectorless.ask.types import TraceStep, Evidence, WorkerOutput, WorkerMetrics
from vectorless.llm_client import LLMClient
from vectorless.ask.tools import compare_nodes, summarize_section, trace_reasoning
from vectorless.ask.prompts import (
    NavigationParams,
    WorkerDispatchParams,
    build_plan_prompt,
    build_replan_prompt,
    check_sufficiency,
    parse_sufficiency_response,
    worker_dispatch,
    worker_navigation,
)

logger = logging.getLogger(__name__)

MAX_HISTORY_ENTRIES = 6


# ---------------------------------------------------------------------------
# Command parsing
# ---------------------------------------------------------------------------

@dataclass
class Command:
    """Parsed command from LLM output."""
    kind: str  # ls, cd, cd_up, cat, find, findtree, grep, head, wc, pwd, check, done, ...
    target: str = ""
    target_b: str = ""  # second target for compare, pattern for grep_node
    lines: int = 20


def _strip_quotes(s: str) -> str:
    """Strip surrounding quotes (straight and smart) from a string."""
    trimmed = s.strip()
    if len(trimmed) < 2:
        return trimmed
    first, last = trimmed[0], trimmed[-1]
    matching = (
        (first == '"' and last == '"')
        or (first == "'" and last == "'")
        or (first == "“" and last == "”")
        or (first == "‘" and last == "’")
    )
    return trimmed[1:-1] if matching else trimmed


def parse_command(llm_output: str) -> Command:
    """Parse the first non-empty line of LLM output into a Command."""
    line = ""
    for l in llm_output.splitlines():
        if l.strip():
            line = l.strip()
            break

    # Remove common wrapping
    line = line.strip().strip("`").strip()

    parts = line.split()
    if not parts:
        return Command(kind="ls")

    cmd = parts[0].lower()

    if cmd == "ls":
        return Command(kind="ls")
    elif cmd == "cd":
        if len(parts) >= 2 and parts[1] == "..":
            return Command(kind="cd_up")
        target = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="cd", target=target)
    elif cmd == "cat":
        target = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else "."
        return Command(kind="cat", target=target)
    elif cmd == "find":
        keyword = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="find", target=keyword)
    elif cmd == "findtree":
        pattern = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="findtree", target=pattern)
    elif cmd == "grep":
        pattern = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="grep", target=pattern)
    elif cmd == "head":
        if len(parts) >= 4 and parts[1] == "-n":
            target = _strip_quotes(" ".join(parts[3:]))
            try:
                n = int(parts[2])
            except ValueError:
                n = 20
            return Command(kind="head", target=target, lines=n)
        target = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="head", target=target)
    elif cmd == "wc":
        target = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="wc", target=target)
    elif cmd == "pwd":
        return Command(kind="pwd")
    elif cmd == "check":
        return Command(kind="check")
    elif cmd == "done":
        return Command(kind="done")
    elif cmd == "back":
        return Command(kind="back")
    elif cmd == "toc":
        if len(parts) > 1:
            try:
                return Command(kind="toc", lines=int(parts[1]))
            except ValueError:
                pass
        return Command(kind="toc", lines=0)  # 0 = no depth limit
    elif cmd == "stats":
        target = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="stats", target=target)
    elif cmd == "grep_node":
        # grep_node <target> <pattern>
        if len(parts) >= 3:
            return Command(kind="grep_node", target=parts[1], target_b=_strip_quotes(" ".join(parts[2:])))
        elif len(parts) == 2:
            return Command(kind="grep_node", target=parts[1])
        return Command(kind="grep_node")
    elif cmd == "similar":
        target = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="similar", target=target)
    elif cmd in ("section_overview", "overview"):
        target = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="section_overview", target=target)
    elif cmd == "compare":
        # compare <node_a> <node_b> — use node IDs for reliability
        if len(parts) >= 3:
            return Command(kind="compare", target=parts[1], target_b=parts[2])
        elif len(parts) == 2:
            return Command(kind="compare", target=parts[1])
        return Command(kind="compare")
    elif cmd == "trace":
        target = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="trace", target=target)
    elif cmd == "summarize":
        target = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="summarize", target=target)
    elif cmd == "siblings":
        target = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="siblings", target=target)
    elif cmd == "ancestors":
        target = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="ancestors", target=target)
    elif cmd in ("doc_card", "card"):
        return Command(kind="doc_card")
    elif cmd == "concepts":
        return Command(kind="concepts")
    elif cmd == "find_section":
        target = _strip_quotes(" ".join(parts[1:])) if len(parts) > 1 else ""
        return Command(kind="find_section", target=target)
    else:
        return Command(kind="ls")  # fallback: re-observe


def _is_parse_failure(command: Command, raw_output: str) -> bool:
    """Detect if the parsed command is a fallback (unrecognized input)."""
    trimmed = raw_output.strip()
    return command.kind == "ls" and not trimmed.startswith("ls") and trimmed != ""


# ---------------------------------------------------------------------------
# Step result
# ---------------------------------------------------------------------------

@dataclass
class Step:
    """Result of a single command execution."""
    kind: str  # continue, done, force_done
    reason: str = ""


# ---------------------------------------------------------------------------
# Worker state
# ---------------------------------------------------------------------------

@dataclass
class _WorkerState:
    """Mutable state for a single Worker run."""
    breadcrumb: list[str] = field(default_factory=lambda: ["root"])
    evidence: list[Evidence] = field(default_factory=list)
    visited: set[str] = field(default_factory=set)
    collected_nodes: set[str] = field(default_factory=set)
    remaining: int = 15
    max_rounds: int = 15
    last_feedback: str = ""
    missing_info: str = ""
    history: list[str] = field(default_factory=list)
    plan: str = ""
    check_count: int = 0
    plan_generated: bool = False
    trace_steps: list[TraceStep] = field(default_factory=list)
    llm_calls: int = 0

    def path_str(self) -> str:
        return "/".join(self.breadcrumb)

    def evidence_summary(self) -> str:
        if not self.evidence:
            return "(none)"
        return "\n".join(
            f"- [{e.node_title}] {len(e.content)} chars" for e in self.evidence
        )

    def evidence_for_check(self) -> str:
        if not self.evidence:
            return "(no evidence collected yet)"
        return "\n\n".join(
            f"[{e.node_title}]\n{e.content}" for e in self.evidence
        )

    def history_text(self) -> str:
        if not self.history:
            return "(no history yet)"
        return "\n".join(
            f"{i + 1}. {h}" for i, h in enumerate(self.history)
        )

    def push_history(self, entry: str) -> None:
        if len(self.history) >= MAX_HISTORY_ENTRIES:
            self.history.pop(0)
        self.history.append(entry)

    def visited_titles(self, doc: Any) -> str:
        titles = []
        for node_id in self.visited:
            try:
                title = _node_title_sync(doc, node_id)
                if title:
                    titles.append(title)
            except Exception:
                pass
        return ", ".join(titles) if titles else "(none)"


def _node_title_sync(doc: Any, node_id: str) -> str:
    """Get node title (for visited titles formatting). Returns empty string on error."""
    try:
        import asyncio
        loop = asyncio.get_event_loop()
        if loop.is_running():
            return ""
        return loop.run_until_complete(doc.node_title(node_id))
    except Exception:
        return ""


async def _resolve_target(doc: Any, target: str, state: _WorkerState) -> str | None:
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


# ---------------------------------------------------------------------------
# Command execution
# ---------------------------------------------------------------------------

async def _execute_command(
    command: Command,
    doc: Any,
    state: _WorkerState,
    query: str,
    llm: LLMClient,
) -> Step:
    """Execute a parsed command against the PyDocument. Returns Step result."""
    kind = command.kind

    if kind == "ls":
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

    elif kind == "cd":
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

    elif kind == "cd_up":
        try:
            await doc.cd_up()
            if len(state.breadcrumb) > 1:
                state.breadcrumb.pop()
            state.last_feedback = f"Current position: /{state.path_str()}"
        except Exception as e:
            state.last_feedback = f"Cannot go up: {e}"
        return Step(kind="continue")

    elif kind == "cat":
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

    elif kind == "find":
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

    elif kind == "findtree":
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

    elif kind == "grep":
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

    elif kind == "head":
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

    elif kind == "wc":
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

    elif kind == "pwd":
        try:
            pwd = await doc.pwd()
            state.last_feedback = f"/{pwd}"
        except Exception as e:
            state.last_feedback = f"pwd error: {e}"
        return Step(kind="continue")

    elif kind == "back":
        try:
            await doc.back()
            pwd = await doc.pwd()
            state.breadcrumb = [p for p in pwd.split("/") if p]
            state.last_feedback = f"Current position: /{state.path_str()}"
        except Exception as e:
            state.last_feedback = f"Cannot go back: {e}"
        return Step(kind="continue")

    elif kind == "toc":
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

    elif kind == "stats":
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

    elif kind == "grep_node":
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

    elif kind == "similar":
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

    elif kind == "section_overview":
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

    elif kind == "compare":
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

    elif kind == "trace":
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

    elif kind == "summarize":
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

    elif kind == "siblings":
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

    elif kind == "ancestors":
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
                        f"  {'  ' * a.depth}→ {a.title} (depth {a.depth}, {a.child_count} children)"
                    )
                state.last_feedback = "\n".join(lines)
        except Exception as e:
            state.last_feedback = f"ancestors error: {e}"
        return Step(kind="continue")

    elif kind == "doc_card":
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

    elif kind == "concepts":
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

    elif kind == "find_section":
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

    elif kind == "check":
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

    elif kind == "done":
        state.last_feedback = "Navigation complete."
        return Step(kind="done")

    else:
        state.last_feedback = f"Unknown command: {kind}"
        return Step(kind="continue")


# ---------------------------------------------------------------------------
# Worker
# ---------------------------------------------------------------------------

class Worker:
    """Navigates a single document to collect evidence for a query.

    Usage::

        worker = Worker(document=doc, query="What is the revenue?", llm_client=llm)
        result = await worker.run()
    """

    def __init__(
        self,
        document: Any,
        query: str,
        llm_client: LLMClient,
        *,
        max_rounds: int = 15,
        max_llm_calls: int = 0,
        task: str | None = None,
        intent_context: str = "",
    ) -> None:
        self._doc = document
        self._query = query
        self._llm = llm_client
        self._max_rounds = max_rounds
        self._max_llm_calls = max_llm_calls
        self._task = task
        self._intent_context = intent_context

    async def run(self) -> WorkerOutput:
        """Execute the Worker navigation loop and return collected evidence."""
        doc = self._doc
        query = self._query
        llm = self._llm
        task = self._task
        max_rounds = self._max_rounds
        max_llm = self._max_llm_calls
        intent_context = self._intent_context

        state = _WorkerState(remaining=max_rounds, max_rounds=max_rounds)

        # Phase 0: initial ls to observe environment
        root_id = await doc.root_id()
        state.visited.add(root_id)

        try:
            children = await doc.ls()
            if children:
                lines = []
                for i, child in enumerate(children, 1):
                    lines.append(
                        f"[{i}] {child.title} — "
                        f"(depth {child.depth}, {child.leaf_count} leaves)"
                    )
                state.last_feedback = "\n".join(lines)
            else:
                state.last_feedback = "(no children at root)"
        except Exception as e:
            state.last_feedback = f"Initial ls failed: {e}"

        # Phase 1.5: optional navigation planning
        keyword_hints = ""
        try:
            keyword_hints = await self._build_keyword_hints(doc, query)
        except Exception:
            pass

        if keyword_hints:
            await self._generate_plan(doc, query, task, state, keyword_hints, llm)

        # Phase 2: main navigation loop
        use_dispatch = task is not None

        while state.remaining > 0:
            if max_llm > 0 and state.llm_calls >= max_llm:
                logger.info("LLM call budget exhausted (%d/%d)", state.llm_calls, max_llm)
                break

            # Build prompt
            if use_dispatch and state.remaining == max_rounds:
                system, user = worker_dispatch(WorkerDispatchParams(
                    original_query=query,
                    task=task or query,
                    doc_name=await doc.doc_name(),
                    breadcrumb=state.path_str(),
                ))
            else:
                visited_titles = state.visited_titles(doc)
                system, user = worker_navigation(NavigationParams(
                    query=query,
                    task=task,
                    breadcrumb=state.path_str(),
                    evidence_summary=state.evidence_summary(),
                    missing_info=state.missing_info,
                    last_feedback=state.last_feedback,
                    remaining=state.remaining,
                    max_rounds=state.max_rounds,
                    history=state.history_text(),
                    visited_titles=visited_titles,
                    plan=state.plan,
                    intent_context=intent_context,
                    keyword_hints=keyword_hints,
                ))

            # LLM decision
            round_num = max_rounds - state.remaining + 1
            try:
                llm_output = await llm.complete(system, user)
            except Exception as e:
                logger.error("LLM call failed at round %d: %s", round_num, e)
                break
            state.llm_calls += 1

            # Parse command
            command = parse_command(llm_output)
            is_failure = _is_parse_failure(command, llm_output)

            if is_failure:
                raw_preview = llm_output.strip()[:200]
                if len(llm_output.strip()) > 200:
                    raw_preview += "..."
                state.last_feedback = (
                    f"Your output was not recognized as a valid command:\n"
                    f'"{raw_preview}"\n\n'
                    f"Please output exactly one command "
                    f"(ls, cd, cat, head, find, grep, toc, stats, similar, overview, "
                    f"siblings, ancestors, doc_card, concepts, find_section, "
                    f"compare, trace, summarize, wc, pwd, check, or done)."
                )
                state.push_history("(unrecognized) → parse failure")
                continue

            is_check = command.kind == "check"

            # Execute
            step = await _execute_command(command, doc, state, query, llm)

            # Re-plan after insufficient check
            if is_check:
                await self._handle_replan(query, task, doc, state, llm, max_llm)

            # Record history and trace
            cmd_str = command.kind
            if command.target:
                cmd_str += f" {command.target}"

            feedback_preview = state.last_feedback
            if len(feedback_preview) > 120:
                feedback_preview = feedback_preview[:120] + "..."
            state.push_history(f"{cmd_str} → {feedback_preview}")

            round_num_done = max_rounds - state.remaining
            state.trace_steps.append(TraceStep(
                action=cmd_str,
                observation=state.last_feedback[:200],
                round=round_num_done,
            ))

            # Check termination
            if step.kind == "done":
                break
            elif step.kind == "force_done":
                break
            else:
                if not is_check:
                    state.remaining -= 1

        budget_exhausted = state.remaining == 0
        rounds_used = max_rounds - state.remaining
        evidence_chars = sum(len(e.content) for e in state.evidence)

        doc_name = ""
        try:
            doc_name = await doc.doc_name()
        except Exception:
            pass

        return WorkerOutput(
            evidence=list(state.evidence),
            metrics=WorkerMetrics(
                rounds_used=rounds_used,
                llm_calls=state.llm_calls,
                nodes_visited=len(state.visited),
                budget_exhausted=budget_exhausted,
                plan_generated=state.plan_generated,
                check_count=state.check_count,
                evidence_chars=evidence_chars,
            ),
            doc_name=doc_name,
            trace_steps=list(state.trace_steps),
        )

    async def _build_keyword_hints(self, doc: Any, query: str) -> str:
        """Build keyword hints from the document's reasoning index."""
        # Extract simple keywords from the query
        stop_words = {
            "what", "is", "the", "a", "an", "how", "does", "do", "are",
            "in", "on", "at", "to", "for", "of", "with", "and", "or",
            "this", "that", "it", "from", "by", "was", "were", "be",
        }
        words = re.findall(r"\b\w+\b", query.lower())
        keywords = [w for w in words if w not in stop_words and len(w) > 2]

        if not keywords:
            return ""

        hints = []
        for kw in keywords[:5]:  # limit keywords
            try:
                entries = await doc.keyword_entries(kw)
                for entry in entries[:3]:
                    title = await doc.node_title(entry.node_id)
                    hints.append(
                        f"  - '{kw}' → {title} (weight {entry.weight:.2f})"
                    )
            except Exception:
                pass

        if not hints:
            return ""

        return "Keyword matches (use find <keyword> to jump directly):\n" + "\n".join(hints) + "\n"

    async def _generate_plan(
        self,
        doc: Any,
        query: str,
        task: str | None,
        state: _WorkerState,
        keyword_hints: str,
        llm: LLMClient,
    ) -> None:
        """Phase 1.5: generate a navigation plan from keyword hints."""
        ls_output = state.last_feedback
        doc_name = await doc.doc_name()

        system, user = build_plan_prompt(
            query=query,
            ls_output=ls_output,
            doc_name=doc_name,
            keyword_hints_section=f"\n{keyword_hints}" if keyword_hints else "",
            task=task,
        )

        try:
            plan = await llm.complete(system, user)
            state.llm_calls += 1
            plan_text = plan.strip()
            if plan_text:
                state.plan = plan_text
                state.plan_generated = True
        except Exception as e:
            logger.warning("Plan generation failed: %s", e)

    async def _handle_replan(
        self,
        query: str,
        task: str | None,
        doc: Any,
        state: _WorkerState,
        llm: LLMClient,
        max_llm: int,
    ) -> None:
        """Dynamic re-planning after an insufficient check."""
        if not state.missing_info:
            return

        if state.remaining < 3:
            state.plan = ""
            state.missing_info = ""
            return

        if max_llm > 0 and state.llm_calls >= max_llm:
            state.plan = ""
            state.missing_info = ""
            return

        # Build sibling hints
        sibling_hints = ""
        current_children = "Current position is a leaf node — consider cd .. to go back.\n"

        try:
            children = await doc.ls()
            if children:
                items = [f"  - {c.title} ({c.leaf_count} leaves)" for c in children]
                current_children = f"Children at current position:\n" + "\n".join(items) + "\n"
        except Exception:
            pass

        system, user = build_replan_prompt(
            query=query,
            task=task,
            path_str=state.path_str(),
            evidence_summary=state.evidence_summary(),
            missing_info=state.missing_info,
            visited_titles=state.visited_titles(doc),
            current_children=current_children,
            sibling_hints=sibling_hints,
            remaining=state.remaining,
            max_rounds=state.max_rounds,
        )

        try:
            new_plan = await llm.complete(system, user)
            state.llm_calls += 1
            plan_text = new_plan.strip()
            if plan_text:
                logger.info("Re-plan generated: %s", plan_text[:200])
                state.plan = plan_text
        except Exception as e:
            logger.warning("Re-plan LLM call failed: %s", e)

        state.missing_info = ""
