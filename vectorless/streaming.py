"""Streaming query results with real-time progress events.

Uses asyncio.Queue for producer-consumer pattern.
Events are emitted at each stage of the Python strategy pipeline:
  understanding_done → workers_dispatched → worker_step → evaluation_done → synthesis_done → completed

Terminal events are ``'completed'`` (with results) or ``'error'``.
"""

from __future__ import annotations

import asyncio
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

    def __init__(self, queue: asyncio.Queue[Optional[Dict]]) -> None:
        self._queue = queue
        self._result: Optional[QueryResponse] = None
        self._consumed = False

    @classmethod
    def from_engine(
        cls,
        engine: Any,
        question: str,
        doc_ids: Optional[List[str]],
        workspace_scope: bool,
    ) -> StreamingQueryResult:
        """Create a StreamingQueryResult that runs the engine pipeline in background."""
        queue: asyncio.Queue[Optional[Dict]] = asyncio.Queue()
        instance = cls(queue)

        async def _run() -> None:
            try:
                result = await engine._ask_python(
                    question, doc_ids, workspace_scope,
                    event_queue=queue,
                )
                instance._result = result
                await queue.put({
                    "type": "completed",
                    "total_results": len(result.items),
                    "confidence": result.items[0].confidence if result.items else 0.0,
                })
            except Exception as e:
                await queue.put({
                    "type": "error",
                    "message": str(e),
                })
            # Sentinel: None signals end of stream
            await queue.put(None)

        asyncio.ensure_future(_run())
        return instance

    def __aiter__(self) -> AsyncIterator[Dict]:
        return self._iterate()

    async def _iterate(self) -> AsyncIterator[Dict]:
        if self._consumed:
            return
        self._consumed = True

        while True:
            event = await self._queue.get()
            if event is None:
                # Sentinel — producer is done
                break
            yield event
            if event.get("type") in ("completed", "error"):
                break

    @property
    def result(self) -> Optional[QueryResponse]:
        """Final result, available after iteration completes."""
        return self._result if self._consumed else None
