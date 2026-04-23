"""Async LLM client for the Python strategy layer."""

from __future__ import annotations

import json
import re
from typing import Any

from openai import AsyncOpenAI


class LLMClient:
    """Lightweight async LLM client based on the OpenAI SDK.

    Supports any OpenAI-compatible endpoint (OpenAI, Azure, local models, etc.)
    """

    def __init__(
        self,
        api_key: str,
        model: str,
        endpoint: str | None = None,
    ) -> None:
        self._model = model
        self._client = AsyncOpenAI(
            api_key=api_key,
            base_url=endpoint,
        )

    @property
    def model(self) -> str:
        return self._model

    async def complete(self, system: str, user: str) -> str:
        """Send a completion request and return the assistant message text."""
        response = await self._client.chat.completions.create(
            model=self._model,
            messages=[
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        )
        return response.choices[0].message.content or ""

    async def complete_json(self, system: str, user: str) -> dict[str, Any]:
        """Send a completion request and parse the response as JSON."""
        text = await self.complete(system, user)
        return _extract_json(text)

    async def complete_with_messages(
        self, messages: list[dict[str, str]]
    ) -> str:
        """Send a completion request with pre-built messages."""
        response = await self._client.chat.completions.create(
            model=self._model,
            messages=messages,  # type: ignore[arg-type]
        )
        return response.choices[0].message.content or ""


def _extract_json(text: str) -> dict[str, Any]:
    """Extract a JSON object from LLM output.

    Handles:
    - Plain JSON
    - JSON wrapped in ```json ... ``` code blocks
    - JSON with leading/trailing text
    """
    # Try code block first
    match = re.search(r"```(?:json)?\s*\n?(.*?)```", text, re.DOTALL)
    if match:
        text = match.group(1).strip()

    # Try to find a top-level { ... }
    start = text.find("{")
    if start != -1:
        # Find the matching closing brace
        depth = 0
        for i in range(start, len(text)):
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
                if depth == 0:
                    candidate = text[start : i + 1]
                    try:
                        return json.loads(candidate)
                    except json.JSONDecodeError:
                        break

    # Last resort: try the whole text
    return json.loads(text.strip())
