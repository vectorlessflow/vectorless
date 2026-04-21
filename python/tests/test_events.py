"""Tests for the event system."""

from __future__ import annotations

from vectorless.events import (
    EventEmitter,
    IndexEventData,
    IndexEventType,
    QueryEventData,
    QueryEventType,
)


class TestEventEmitter:
    def test_index_events(self):
        received = []
        emitter = EventEmitter()

        @emitter.on_index
        def handler(event):
            received.append(event)

        event = IndexEventData(
            event_type=IndexEventType.STARTED,
            path="/test/doc.pdf",
        )
        emitter.emit_index(event)

        assert len(received) == 1
        assert received[0].path == "/test/doc.pdf"
        assert received[0].event_type == IndexEventType.STARTED

    def test_query_events(self):
        received = []
        emitter = EventEmitter()

        @emitter.on_query
        def handler(event):
            received.append(event)

        event = QueryEventData(
            event_type=QueryEventType.COMPLETE,
            query="What is revenue?",
            total_results=3,
        )
        emitter.emit_query(event)

        assert len(received) == 1
        assert received[0].query == "What is revenue?"
        assert received[0].total_results == 3

    def test_multiple_handlers(self):
        count = [0]
        emitter = EventEmitter()

        emitter.on_index(lambda e: count.__setitem__(0, count[0] + 1))
        emitter.on_index(lambda e: count.__setitem__(0, count[0] + 1))

        emitter.emit_index(
            IndexEventData(event_type=IndexEventType.COMPLETE)
        )

        assert count[0] == 2

    def test_chaining(self):
        emitter = EventEmitter()
        result = emitter.on_index(lambda e: None)
        assert result is emitter

    def test_no_handlers(self):
        emitter = EventEmitter()
        # Should not raise
        emitter.emit_index(
            IndexEventData(event_type=IndexEventType.COMPLETE)
        )
        emitter.emit_query(
            QueryEventData(event_type=QueryEventType.COMPLETE)
        )
