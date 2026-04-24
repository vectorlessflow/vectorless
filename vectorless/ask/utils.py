"""Shared utilities for the ask pipeline."""

from __future__ import annotations

import json
import re


def parse_json_response(response: str) -> dict:
    """Parse LLM response as JSON, handling markdown-wrapped output."""
    trimmed = response.strip()

    if trimmed.startswith("```"):
        match = re.search(r"```(?:json)?\s*\n?(.*?)```", trimmed, re.DOTALL)
        if match:
            trimmed = match.group(1).strip()

    start = trimmed.find("{")
    if start != -1:
        depth = 0
        for i in range(start, len(trimmed)):
            if trimmed[i] == "{":
                depth += 1
            elif trimmed[i] == "}":
                depth -= 1
                if depth == 0:
                    candidate = trimmed[start : i + 1]
                    try:
                        return json.loads(candidate)
                    except json.JSONDecodeError:
                        break

    return json.loads(trimmed)
