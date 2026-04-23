"""Async LLM client for the Python strategy layer.

Uses litellm for multi-provider support (OpenAI, Anthropic, DeepSeek, etc.)
and instructor for structured output validation.

Features:
- Unified interface via litellm (100+ providers)
- Structured JSON output via instructor + Pydantic
- Automatic retry with feedback on validation failure
- Per-request timeout
- In-memory response cache (optional, per-session dedup)
"""

from __future__ import annotations

import hashlib
import json
import logging
from typing import Any, Optional, Type, TypeVar

import litellm
from pydantic import BaseModel

logger = logging.getLogger(__name__)

T = TypeVar("T", bound=BaseModel)

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------

DEFAULT_MAX_RETRIES = 2
DEFAULT_TIMEOUT = 120.0  # seconds

# ---------------------------------------------------------------------------
# LLMClient
# ---------------------------------------------------------------------------


class LLMClient:
    """Async LLM client backed by litellm.

    Supports any provider litellm supports (OpenAI, Anthropic, DeepSeek, etc.)
    via model prefix conventions (e.g. "openai/gpt-4o", "anthropic/claude-sonnet-4").

    Usage::

        llm = LLMClient(api_key="sk-...", model="gpt-4o")
        text = await llm.complete("You are a helpful assistant", "What is 2+2?")

        # Structured output
        class MyResponse(BaseModel):
            answer: str
            confidence: float
        result = await llm.complete_structured("...", "...", MyResponse)
        print(result.answer)
    """

    def __init__(
        self,
        api_key: str,
        model: str,
        endpoint: Optional[str] = None,
        *,
        max_retries: int = DEFAULT_MAX_RETRIES,
        timeout: float = DEFAULT_TIMEOUT,
        enable_cache: bool = True,
    ) -> None:
        self._model = model
        self._api_key = api_key
        self._endpoint = endpoint
        self._max_retries = max_retries
        self._timeout = timeout
        self._cache: dict[str, str] = {} if enable_cache else {}
        self._cache_enabled = enable_cache

        # Configure litellm defaults
        if endpoint:
            litellm.api_base = endpoint

    @property
    def model(self) -> str:
        return self._model

    # ── Core completion ──────────────────────────────────────────

    async def complete(
        self,
        system: str,
        user: str,
        *,
        temperature: float = 0.0,
        timeout: Optional[float] = None,
    ) -> str:
        """Send a completion request and return the assistant message text.

        Args:
            system: System prompt.
            user: User message.
            temperature: Sampling temperature.
            timeout: Per-request timeout in seconds (overrides default).

        Returns:
            The assistant's text response.
        """
        cache_key = self._cache_key(system, user, temperature)
        if self._cache_enabled and cache_key in self._cache:
            return self._cache[cache_key]

        response = await self._call_with_retry(
            system=system,
            user=user,
            temperature=temperature,
            timeout=timeout or self._timeout,
        )

        if self._cache_enabled:
            self._cache[cache_key] = response

        return response

    async def complete_json(
        self,
        system: str,
        user: str,
        *,
        temperature: float = 0.0,
        timeout: Optional[float] = None,
    ) -> dict[str, Any]:
        """Send a completion request and parse the response as JSON.

        Falls back to regex extraction if the response is not valid JSON.
        """
        text = await self.complete(system, user, temperature=temperature, timeout=timeout)
        return _extract_json(text)

    async def complete_structured(
        self,
        system: str,
        user: str,
        response_model: Type[T],
        *,
        max_retries: Optional[int] = None,
        temperature: float = 0.0,
        timeout: Optional[float] = None,
    ) -> T:
        """Send a completion request with structured output via instructor.

        Uses instructor's `from_litellm` to get typed Pydantic responses.
        On validation failure, automatically retries with error feedback.

        Args:
            system: System prompt.
            user: User message.
            response_model: Pydantic model class for the expected response.
            max_retries: Max retries on validation failure (overrides default).
            temperature: Sampling temperature.
            timeout: Per-request timeout in seconds.

        Returns:
            Validated instance of response_model.
        """
        import instructor

        client = instructor.from_litellm(litellm.acompletion)
        retries = max_retries if max_retries is not None else self._max_retries

        messages = [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ]

        return await client.chat.completions.create(
            model=self._model,
            messages=messages,
            response_model=response_model,
            max_retries=retries,
            temperature=temperature,
            timeout=timeout or self._timeout,
            api_key=self._api_key,
            api_base=self._endpoint,
        )

    async def complete_with_messages(
        self,
        messages: list[dict[str, str]],
        *,
        temperature: float = 0.0,
        timeout: Optional[float] = None,
    ) -> str:
        """Send a completion request with pre-built messages."""
        response = await litellm.acompletion(
            model=self._model,
            messages=messages,
            temperature=temperature,
            timeout=timeout or self._timeout,
            api_key=self._api_key,
            api_base=self._endpoint,
        )
        return response.choices[0].message.content or ""

    # ── Internal ─────────────────────────────────────────────────

    async def _call_with_retry(
        self,
        system: str,
        user: str,
        temperature: float,
        timeout: float,
    ) -> str:
        """Call litellm.acompletion with retry on transient errors."""
        messages = [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ]

        last_error: Optional[Exception] = None
        for attempt in range(1 + self._max_retries):
            try:
                response = await litellm.acompletion(
                    model=self._model,
                    messages=messages,
                    temperature=temperature,
                    timeout=timeout,
                    api_key=self._api_key,
                    api_base=self._endpoint,
                )
                return response.choices[0].message.content or ""
            except litellm.RateLimitError as e:
                last_error = e
                logger.warning("LLM rate limit hit, attempt %d/%d: %s", attempt + 1, self._max_retries + 1, e)
                if attempt < self._max_retries:
                    import asyncio
                    await asyncio.sleep(2 ** attempt)
            except litellm.Timeout as e:
                last_error = e
                logger.warning("LLM timeout, attempt %d/%d", attempt + 1, self._max_retries + 1)
            except litellm.APIConnectionError as e:
                last_error = e
                logger.warning("LLM connection error, attempt %d/%d: %s", attempt + 1, self._max_retries + 1, e)

        raise LLMError(f"LLM call failed after {self._max_retries + 1} attempts: {last_error}") from last_error

    def _cache_key(self, system: str, user: str, temperature: float) -> str:
        raw = f"{self._model}:{temperature}:{system}|||{user}"
        return hashlib.sha256(raw.encode()).hexdigest()

    def clear_cache(self) -> None:
        """Clear the in-memory response cache."""
        if self._cache_enabled:
            self._cache.clear()


# ---------------------------------------------------------------------------
# Exceptions
# ---------------------------------------------------------------------------


class LLMError(Exception):
    """Raised when an LLM call fails after all retries."""


# ---------------------------------------------------------------------------
# JSON extraction fallback
# ---------------------------------------------------------------------------


def _extract_json(text: str) -> dict[str, Any]:
    """Extract a JSON object from LLM output.

    Handles:
    - Plain JSON
    - JSON wrapped in ```json ... ``` code blocks
    - JSON with leading/trailing text
    """
    import re

    match = re.search(r"```(?:json)?\s*\n?(.*?)```", text, re.DOTALL)
    if match:
        text = match.group(1).strip()

    start = text.find("{")
    if start != -1:
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

    return json.loads(text.strip())
