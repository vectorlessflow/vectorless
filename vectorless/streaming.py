"""Streaming query results backed by real-time Rust streaming events.

Wraps the PyO3 ``StreamingQuery`` async iterator and builds a
``QueryResponse`` from the terminal ``completed`` event.
"""

from __future__ import annotations

from typing import Any, AsyncIterator, Dict, List, Optional

from vectorless.types.results import QueryResponse, QueryResult


class StreamingQueryResult:
    """Async iterator for real-time query progress events.

    Usage::

        stream = await session.query_stream("What is the revenue?")
        async for event in stream:
            print(event["type"], event)
        result = stream.result  # Available after iteration completes
    """

    def __init__(self, raw_stream: Any) -> None:
        self._stream = raw_stream  # PyStreamingQuery from Rust
        self._result: Optional[QueryResponse] = None
        self._consumed = False

    def __aiter__(self) -> AsyncIterator[Dict]:
        return self._iterate()

    async def _iterate(self) -> AsyncIterator[Dict]:
        if self._consumed:
            return
        self._consumed = True

        completed_event: Optional[Dict] = None

        async for event in self._stream:
            event_type = event.get("type", "")

            yield event

            if event_type in ("completed", "error"):
                if event_type == "completed":
                    completed_event = event
                break  # Terminal events end the stream

        if completed_event is not None:
            self._result = self._build_response(completed_event)

    @staticmethod
    def _build_response(event: Dict) -> QueryResponse:
        """Build a QueryResponse from the completed event dict."""
        items: List[QueryResult] = []
        for r in event.get("results", []):
            node_id = r.get("node_id")
            items.append(
                QueryResult(
                    doc_id=node_id or "",
                    content=r.get("content") or "",
                    score=r.get("score", 0.0),
                    confidence=event.get("confidence", 0.0),
                    node_ids=[node_id] if node_id else [],
                    evidence=[],
                    metrics=None,
                )
            )
        return QueryResponse(items=items, failed=[])

    @property
    def result(self) -> Optional[QueryResponse]:
        """Final result, available after iteration completes."""
        return self._result if self._consumed else None
