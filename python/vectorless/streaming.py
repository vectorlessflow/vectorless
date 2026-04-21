"""Streaming query compatibility layer.

Provides an async iterator interface for queries. Currently wraps the
synchronous ``query()`` and yields synthetic progress events. Real-time
streaming requires exposing ``query_stream()`` from Rust via PyO3.
"""

from __future__ import annotations

from typing import AsyncIterator, Dict, List, Optional

from vectorless.types.results import QueryResponse


class StreamingQueryResult:
    """Async iterator for query progress events.

    Usage::

        stream = await session.query_stream("What is the revenue?")
        async for event in stream:
            print(event)
        result = stream.result
    """

    def __init__(self, response: QueryResponse) -> None:
        self._response = response
        self._consumed = False

    def __aiter__(self) -> AsyncIterator[Dict]:
        return self._iterate()

    async def _iterate(self) -> AsyncIterator[Dict]:
        if self._consumed:
            return
        self._consumed = True

        # Synthetic events from the final result
        yield {"type": "started", "message": "Query started"}

        for i, item in enumerate(self._response.items):
            yield {
                "type": "candidate_found",
                "doc_id": item.doc_id,
                "score": item.score,
                "confidence": item.confidence,
                "index": i,
            }

            for j, evidence in enumerate(item.evidence):
                yield {
                    "type": "evidence",
                    "doc_id": item.doc_id,
                    "evidence_title": evidence.title,
                    "evidence_path": evidence.path,
                    "content_length": len(evidence.content),
                    "index": j,
                }

        if self._response.has_failures():
            for failed in self._response.failed:
                yield {
                    "type": "error",
                    "source": failed.source,
                    "error": failed.error,
                }

        yield {
            "type": "completed",
            "total_results": len(self._response.items),
            "total_failures": len(self._response.failed),
        }

    @property
    def result(self) -> Optional[QueryResponse]:
        """Final result, available after iteration completes."""
        return self._response if self._consumed else None
