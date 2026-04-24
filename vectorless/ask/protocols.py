"""Protocol definitions for callable parameters.

Replaces loose `Any` / `Callable` typing with structural typing via Protocol.
"""

from __future__ import annotations

from typing import Any, Protocol


class DocLoader(Protocol):
    """Async callable that loads a navigable document by ID."""

    async def __call__(self, doc_id: str) -> Any: ...


class EventCallback(Protocol):
    """Async callable that receives event dicts."""

    async def __call__(self, event: dict) -> Any: ...
