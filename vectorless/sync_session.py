"""Synchronous Vectorless Session API.

``SyncSession`` provides the same API as ``Session`` but with synchronous
methods — no ``async``/``await`` required. Works in scripts, Jupyter
notebooks, and any synchronous Python context.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, List, Optional, Union

from vectorless._async_utils import run_async
from vectorless.config import EngineConfig, load_config_from_env, load_config_from_file
from vectorless.events import EventEmitter
from vectorless.session import Session
from vectorless.streaming import StreamingQueryResult
from vectorless.types.graph import DocumentGraphWrapper
from vectorless.types.results import IndexResultWrapper, QueryResponse


class SyncSession:
    """Synchronous Vectorless session.

    Same API as ``Session`` but all methods are blocking (no async/await).
    Works in Jupyter notebooks, scripts, and synchronous contexts.

    Usage::

        from vectorless import SyncSession

        session = SyncSession(api_key="sk-...", model="gpt-4o")
        result = session.index(path="./report.pdf")
        answer = session.ask("What is the Q4 revenue?", doc_ids=[result.doc_id])
        print(answer.single().content)
    """

    def __init__(
        self,
        api_key: Optional[str] = None,
        model: Optional[str] = None,
        endpoint: Optional[str] = None,
        config: Optional[EngineConfig] = None,
        config_file: Optional[Union[str, Path]] = None,
        events: Optional[EventEmitter] = None,
    ) -> None:
        self._session = Session(
            api_key=api_key,
            model=model,
            endpoint=endpoint,
            config=config,
            config_file=config_file,
            events=events,
        )

    @classmethod
    def from_env(cls, events: Optional[EventEmitter] = None) -> "SyncSession":
        """Create a SyncSession from environment variables."""
        config = load_config_from_env()
        return cls(config=config, events=events)

    @classmethod
    def from_config_file(
        cls,
        path: Union[str, Path],
        events: Optional[EventEmitter] = None,
    ) -> "SyncSession":
        """Create a SyncSession from a TOML config file."""
        config = load_config_from_file(Path(path))
        return cls(config=config, events=events)

    # ── Indexing ──────────────────────────────────────────────

    def index(
        self,
        path: Optional[Union[str, Path]] = None,
        paths: Optional[List[Union[str, Path]]] = None,
        directory: Optional[Union[str, Path]] = None,
        content: Optional[str] = None,
        bytes_data: Optional[bytes] = None,
        format: str = "markdown",
        name: Optional[str] = None,
        mode: str = "default",
        force: bool = False,
    ) -> IndexResultWrapper:
        """Index a document (synchronous).

        Exactly one source must be provided: path, paths, directory,
        content, or bytes_data.
        """
        return run_async(
            self._session.index(
                path=path,
                paths=paths,
                directory=directory,
                content=content,
                bytes_data=bytes_data,
                format=format,
                name=name,
                mode=mode,
                force=force,
            )
        )

    def index_batch(
        self,
        paths: List[Union[str, Path]],
        *,
        mode: str = "default",
        jobs: int = 1,
        force: bool = False,
        progress: bool = True,
    ) -> List[IndexResultWrapper]:
        """Index multiple files with optional concurrency (synchronous)."""
        return run_async(
            self._session.index_batch(
                paths,
                mode=mode,
                jobs=jobs,
                force=force,
                progress=progress,
            )
        )

    # ── Querying ──────────────────────────────────────────────

    def ask(
        self,
        question: str,
        *,
        doc_ids: Optional[List[str]] = None,
        workspace_scope: bool = False,
        timeout_secs: Optional[int] = None,
    ) -> QueryResponse:
        """Ask a question and get results with source attribution (synchronous)."""
        return run_async(
            self._session.ask(
                question,
                doc_ids=doc_ids,
                workspace_scope=workspace_scope,
                timeout_secs=timeout_secs,
            )
        )

    def query_stream(
        self,
        question: str,
        **kwargs: Any,
    ) -> StreamingQueryResult:
        """Start a streaming query (synchronous).

        Returns a ``StreamingQueryResult`` that is consumed as an async
        iterator. For fully synchronous queries, use ``ask()`` instead.
        """
        return run_async(self._session.query_stream(question, **kwargs))

    # ── Document Management ───────────────────────────────────

    def list_documents(self) -> list:
        """List all indexed documents."""
        return run_async(self._session.list_documents())

    def remove_document(self, doc_id: str) -> bool:
        """Remove a document by ID."""
        return run_async(self._session.remove_document(doc_id))

    def document_exists(self, doc_id: str) -> bool:
        """Check if a document exists."""
        return run_async(self._session.document_exists(doc_id))

    def clear_all(self) -> int:
        """Remove all indexed documents. Returns count removed."""
        return run_async(self._session.clear_all())

    # ── Graph ─────────────────────────────────────────────────

    def get_graph(self) -> Optional[DocumentGraphWrapper]:
        """Get the cross-document relationship graph."""
        return run_async(self._session.get_graph())

    # ── Metrics ───────────────────────────────────────────────

    def metrics_report(self) -> Any:
        """Get a comprehensive metrics report."""
        return self._session.metrics_report()

    # ── Context Manager ───────────────────────────────────────

    def __enter__(self) -> "SyncSession":
        return self

    def __exit__(self, *args: Any) -> None:
        pass

    def __repr__(self) -> str:
        return f"SyncSession({self._session!r})"
