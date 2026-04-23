"""Streaming query results backed by the Python strategy pipeline.

Yields real-time events as the orchestrator progresses through
understanding, dispatching workers, collecting evidence, and synthesis.
"""

from __future__ import annotations

from typing import Any, AsyncIterator, Dict, List, Optional

from vectorless.types.results import QueryResponse


class StreamingQueryResult:
    """Async iterator for real-time query progress events.

    Usage::

        stream = await engine.query_stream("What is the revenue?")
        async for event in stream:
            print(event["type"], event)
        result = stream.result
    """

    def __init__(self) -> None:
        self._result: Optional[QueryResponse] = None
        self._consumed = False
        self._queue: list[Dict] = []
        self._finished = False

    @classmethod
    def from_engine(
        cls,
        engine: Any,
        question: str,
        doc_ids: Optional[List[str]],
        workspace_scope: bool,
    ) -> StreamingQueryResult:
        """Create a StreamingQueryResult that runs the engine pipeline with event emission."""
        instance = cls()

        async def _run() -> None:
            try:
                result = await engine._ask_python(question, doc_ids, workspace_scope)
                instance._result = result
                instance._queue.append({
                    "type": "completed",
                    "total_results": len(result.items),
                    "confidence": result.items[0].confidence if result.items else 0.0,
                })
            except Exception as e:
                instance._queue.append({
                    "type": "error",
                    "message": str(e),
                })
            finally:
                instance._finished = True

        import asyncio
        asyncio.ensure_future(_run())
        return instance

    def __aiter__(self) -> AsyncIterator[Dict]:
        return self._iterate()

    async def _iterate(self) -> AsyncIterator[Dict]:
        if self._consumed:
            return
        self._consumed = True

        import asyncio
        while True:
            if self._queue:
                event = self._queue.pop(0)
                yield event
                if event.get("type") in ("completed", "error"):
                    break
            elif self._finished:
                break
            else:
                await asyncio.sleep(0.05)

    @property
    def result(self) -> Optional[QueryResponse]:
        """Final result, available after iteration completes."""
        return self._result if self._consumed else None
