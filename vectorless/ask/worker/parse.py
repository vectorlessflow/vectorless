"""Command parsing — converts LLM text output into structured Command objects.

Separated from execution for testability: parse logic has no side effects.
"""

from __future__ import annotations

from dataclasses import dataclass


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
        or (first == "\u201c" and last == "\u201d")
        or (first == "\u2018" and last == "\u2019")
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
