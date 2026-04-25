"""Async LLM client for the Python strategy layer.

Uses litellm for multi-provider support (OpenAI, Anthropic, DeepSeek, etc.)
and instructor for structured output validation.

Features:
- Unified interface via litellm (100+ providers)
- Structured JSON output via instructor + Pydantic
- Automatic retry with feedback on validation failure
- Per-request timeout
- LRU response cache (bounded, per-session dedup)
- Per-call api_base (no global state mutation)
"""

from __future__ import annotations

import hashlib
import json
import logging
from collections import OrderedDict
from typing import Any, Type, TypeVar

import litellm
from pydantic import BaseModel

from vectorless.ask.errors import LLMFailureError, ParseError
from vectorless.ask.utils import parse_json_response

logger = logging.getLogger(__name__)

T = TypeVar("T", bound=BaseModel)

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------

DEFAULT_MAX_RETRIES = 2
DEFAULT_TIMEOUT = 120.0  # seconds
DEFAULT_CACHE_SIZE = 256

# ---------------------------------------------------------------------------
# LRU Cache
# ---------------------------------------------------------------------------


class _LRUCache:
    """Bounded LRU cache for LLM response dedup."""

    def __init__(self, max_size: int = DEFAULT_CACHE_SIZE) -> None:
        self._cache: OrderedDict[str, str] = OrderedDict()
        self._max_size = max_size

    def get(self, key: str) -> str | None:
        if key in self._cache:
            self._cache.move_to_end(key)
            return self._cache[key]
        return None

    def put(self, key: str, value: str) -> None:
        if key in self._cache:
            self._cache.move_to_end(key)
        self._cache[key] = value
        if len(self._cache) > self._max_size:
            self._cache.popitem(last=False)

    def clear(self) -> None:
        self._cache.clear()


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
        endpoint: str | None = None,
        *,
        max_retries: int = DEFAULT_MAX_RETRIES,
        timeout: float = DEFAULT_TIMEOUT,
        enable_cache: bool = True,
        cache_size: int = DEFAULT_CACHE_SIZE,
    ) -> None:
        self._model = model
        self._api_key = api_key
        self._endpoint = endpoint
        self._max_retries = max_retries
        self._timeout = timeout
        self._cache = _LRUCache(cache_size) if enable_cache else None
        self._cache_enabled = enable_cache

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
        timeout: float | None = None,
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
        if self._cache_enabled and self._cache is not None:
            cached = self._cache.get(cache_key)
            if cached is not None:
                return cached

        response = await self._call_with_retry(
            system=system,
            user=user,
            temperature=temperature,
            timeout=timeout or self._timeout,
        )

        if self._cache_enabled and self._cache is not None:
            self._cache.put(cache_key, response)

        return response

    async def complete_json(
        self,
        system: str,
        user: str,
        *,
        temperature: float = 0.0,
        timeout: float | None = None,
    ) -> dict[str, Any]:
        """Send a completion request and parse the response as JSON."""
        text = await self.complete(system, user, temperature=temperature, timeout=timeout)
        try:
            return parse_json_response(text)
        except ValueError as e:
            raise ParseError(str(e), raw_output=text) from e

    async def complete_structured(
        self,
        system: str,
        user: str,
        response_model: Type[T],
        *,
        max_retries: int | None = None,
        temperature: float = 0.0,
        timeout: float | None = None,
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
        timeout: float | None = None,
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
        import asyncio

        messages = [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ]

        last_error: Exception | None = None
        for attempt in range(1 + self._max_retries):
            try:
                logger.info(
                    "LLM call: model=%s endpoint=%s attempt=%d timeout=%.0fs",
                    self._model, self._endpoint, attempt + 1, timeout,
                )
                response = await litellm.acompletion(
                    model=self._model,
                    messages=messages,
                    temperature=temperature,
                    timeout=timeout,
                    api_key=self._api_key,
                    api_base=self._endpoint,
                )
                logger.info("LLM call: response received (%d chars)", len(response.choices[0].message.content or ""))
                return response.choices[0].message.content or ""
            except litellm.RateLimitError as e:
                last_error = e
                logger.warning("LLM rate limit hit, attempt %d/%d: %s", attempt + 1, self._max_retries + 1, e)
                if attempt < self._max_retries:
                    await asyncio.sleep(2 ** attempt)
            except litellm.Timeout as e:
                last_error = e
                logger.warning("LLM timeout, attempt %d/%d", attempt + 1, self._max_retries + 1)
                if attempt < self._max_retries:
                    await asyncio.sleep(1.0)
            except litellm.APIConnectionError as e:
                last_error = e
                logger.warning("LLM connection error, attempt %d/%d: %s", attempt + 1, self._max_retries + 1, e)
                if attempt < self._max_retries:
                    await asyncio.sleep(1.0)

        raise LLMFailureError(
            f"LLM call failed after {self._max_retries + 1} attempts: {last_error}",
            attempts=self._max_retries + 1,
        ) from last_error

    def _cache_key(self, system: str, user: str, temperature: float) -> str:
        raw = f"{self._model}:{temperature}:{system}|||{user}"
        return hashlib.sha256(raw.encode()).hexdigest()

    def clear_cache(self) -> None:
        """Clear the in-memory response cache."""
        if self._cache is not None:
            self._cache.clear()
