"""Python-side event callback system for progress monitoring.

This is a pure-Python callback registry. It fires events based on
result data after operations complete. Real-time streaming events
require the Rust ``query_stream()`` to be exposed via PyO3 (future work).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Callable, List, Optional


class IndexEventType(str, Enum):
    STARTED = "started"
    FORMAT_DETECTED = "format_detected"
    PARSING_PROGRESS = "parsing_progress"
    TREE_BUILT = "tree_built"
    SUMMARY_PROGRESS = "summary_progress"
    COMPLETE = "complete"
    ERROR = "error"


class QueryEventType(str, Enum):
    STARTED = "started"
    NODE_VISITED = "node_visited"
    CANDIDATE_FOUND = "candidate_found"
    SUFFICIENCY_CHECK = "sufficiency_check"
    COMPLETE = "complete"
    ERROR = "error"


@dataclass
class IndexEventData:
    """Data payload for index events."""

    event_type: IndexEventType
    path: Optional[str] = None
    format: Optional[str] = None
    percent: Optional[int] = None
    node_count: Optional[int] = None
    completed: Optional[int] = None
    total: Optional[int] = None
    doc_id: Optional[str] = None
    message: Optional[str] = None


@dataclass
class QueryEventData:
    """Data payload for query events."""

    event_type: QueryEventType
    query: Optional[str] = None
    node_id: Optional[str] = None
    title: Optional[str] = None
    score: Optional[float] = None
    tokens: Optional[int] = None
    total_results: Optional[int] = None
    confidence: Optional[float] = None
    message: Optional[str] = None


IndexEventHandler = Callable[[IndexEventData], None]
QueryEventHandler = Callable[[QueryEventData], None]
WorkspaceEventHandler = Callable[[dict], None]


class EventEmitter:
    """Python-side event emitter for progress monitoring.

    Usage::

        from vectorless import Engine, EventEmitter

        events = EventEmitter()

        @events.on_query
        def on_query(event):
            print(f"Query: {event.query}")

        engine = Engine(api_key="sk-...", model="gpt-4o", events=events)
    """

    def __init__(self) -> None:
        self._index_handlers: List[IndexEventHandler] = []
        self._query_handlers: List[QueryEventHandler] = []
        self._workspace_handlers: List[WorkspaceEventHandler] = []

    def on_index(self, handler: IndexEventHandler) -> "EventEmitter":
        """Register an index event handler. Can be used as decorator."""
        self._index_handlers.append(handler)
        return self

    def on_query(self, handler: QueryEventHandler) -> "EventEmitter":
        """Register a query event handler. Can be used as decorator."""
        self._query_handlers.append(handler)
        return self

    def on_workspace(self, handler: WorkspaceEventHandler) -> "EventEmitter":
        """Register a workspace event handler. Can be used as decorator."""
        self._workspace_handlers.append(handler)
        return self

    def emit_index(self, event: IndexEventData) -> None:
        """Emit an index event to all registered handlers."""
        for handler in self._index_handlers:
            handler(event)

    def emit_query(self, event: QueryEventData) -> None:
        """Emit a query event to all registered handlers."""
        for handler in self._query_handlers:
            handler(event)

    def emit_workspace(self, event: dict) -> None:
        """Emit a workspace event to all registered handlers."""
        for handler in self._workspace_handlers:
            handler(event)
