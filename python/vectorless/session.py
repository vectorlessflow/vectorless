"""High-level Vectorless Session API.

``Session`` is the single recommended entry point for all operations.
It wraps the Rust Engine with Pythonic ergonomics: typed configuration,
event callbacks, flexible input methods, and batch operations.
"""

from __future__ import annotations

import asyncio
from pathlib import Path
from typing import Any, List, Optional, Union

from vectorless._core import Engine, IndexContext, IndexOptions, QueryContext
from vectorless.config import EngineConfig, load_config_from_env
from vectorless.events import (
    EventEmitter,
    IndexEventData,
    IndexEventType,
    QueryEventData,
    QueryEventType,
)
from vectorless.streaming import StreamingQueryResult
from vectorless.types.graph import DocumentGraphWrapper
from vectorless.types.results import (
    IndexResultWrapper,
    QueryResponse,
)


class Session:
    """High-level Vectorless session.

    Configuration precedence: constructor args > env vars > config file > defaults.

    Usage::

        from vectorless import Session

        session = Session(api_key="sk-...", model="gpt-4o")
        result = await session.index(path="./report.pdf")
        answer = await session.ask("What is the Q4 revenue?", doc_ids=[result.doc_id])
        print(answer.single().content)

    Or from environment variables::

        # VECTORLESS_API_KEY, VECTORLESS_MODEL set in env
        session = Session.from_env()
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
        self._events = events or EventEmitter()

        # Resolve config: constructor > env > file > defaults
        if config is not None:
            self._config = config
        else:
            self._config = self._resolve_config(api_key, model, endpoint, config_file)

        # Build Rust engine
        rust_config = self._config.to_rust_config()
        self._engine = Engine(
            api_key=self._config.llm.api_key,
            model=self._config.llm.model or None,
            endpoint=self._config.llm.endpoint or None,
            config=rust_config,
        )

    @classmethod
    def from_env(cls, events: Optional[EventEmitter] = None) -> "Session":
        """Create a Session from environment variables only."""
        config = load_config_from_env()
        return cls(config=config, events=events)

    @classmethod
    def from_config_file(
        cls,
        path: Union[str, Path],
        events: Optional[EventEmitter] = None,
    ) -> "Session":
        """Create a Session from a TOML config file."""
        from vectorless.config import load_config_from_file

        config = load_config_from_file(Path(path))
        return cls(config=config, events=events)

    def _resolve_config(
        self,
        api_key: Optional[str],
        model: Optional[str],
        endpoint: Optional[str],
        config_file: Optional[Union[str, Path]],
    ) -> EngineConfig:
        from vectorless.config import load_config

        overrides: dict[str, Any] = {}
        llm_overrides: dict[str, Any] = {}
        if api_key is not None:
            llm_overrides["api_key"] = api_key
        if model is not None:
            llm_overrides["model"] = model
        if endpoint is not None:
            llm_overrides["endpoint"] = endpoint
        if llm_overrides:
            overrides["llm"] = llm_overrides

        return load_config(
            config_file=Path(config_file) if config_file else None,
            overrides=overrides if overrides else None,
        )

    # ── Indexing ──────────────────────────────────────────────

    async def index(
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
        """Index a document from various sources.

        Exactly one source must be provided: path, paths, directory,
        content, or bytes_data.
        """
        sources_provided = sum(
            x is not None for x in [path, paths, directory, content, bytes_data]
        )
        if sources_provided != 1:
            raise ValueError(
                "Provide exactly one source: path, paths, directory, content, or bytes_data"
            )

        if force:
            mode = "force"

        # Build IndexContext
        if path is not None:
            ctx = IndexContext.from_path(str(path))
        elif paths is not None:
            ctx = IndexContext.from_paths([str(p) for p in paths])
        elif directory is not None:
            ctx = IndexContext.from_dir(str(directory), recursive=True)
        elif content is not None:
            ctx = IndexContext.from_content(content, format)
        elif bytes_data is not None:
            ctx = IndexContext.from_bytes(list(bytes_data), format)
        else:
            raise ValueError("No source provided")

        if name is not None:
            ctx = ctx.with_name(name)
        if mode != "default":
            ctx = ctx.with_mode(mode)

        # Emit start event
        source_desc = str(path or paths or directory or "<content>" or "<bytes>")
        self._events.emit_index(
            IndexEventData(event_type=IndexEventType.STARTED, path=source_desc)
        )

        result = await self._engine.index(ctx)

        # Emit complete event
        self._events.emit_index(
            IndexEventData(
                event_type=IndexEventType.COMPLETE,
                doc_id=result.doc_id,
                message=f"Indexed {result.doc_id or 'documents'}",
            )
        )

        return IndexResultWrapper.from_rust(result)

    async def index_batch(
        self,
        paths: List[Union[str, Path]],
        *,
        mode: str = "default",
        jobs: int = 1,
        force: bool = False,
        progress: bool = True,
    ) -> List[IndexResultWrapper]:
        """Index multiple files with optional concurrency.

        Args:
            paths: List of file paths to index.
            mode: Indexing mode ("default", "force", "incremental").
            jobs: Max concurrent indexing jobs.
            force: Force re-index existing documents.
            progress: Emit progress events.
        """
        semaphore = asyncio.Semaphore(jobs)
        results: List[IndexResultWrapper] = []

        async def _index_one(p: Union[str, Path]) -> IndexResultWrapper:
            async with semaphore:
                self._events.emit_index(
                    IndexEventData(
                        event_type=IndexEventType.STARTED,
                        path=str(p),
                    )
                )
                result = await self.index(path=p, mode=mode, force=force)
                if progress:
                    self._events.emit_index(
                        IndexEventData(
                            event_type=IndexEventType.COMPLETE,
                            path=str(p),
                            doc_id=result.doc_id,
                        )
                    )
                return result

        tasks = [_index_one(p) for p in paths]
        results = await asyncio.gather(*tasks)
        return list(results)

    # ── Querying ──────────────────────────────────────────────

    async def ask(
        self,
        question: str,
        *,
        doc_ids: Optional[List[str]] = None,
        workspace_scope: bool = False,
        timeout_secs: Optional[int] = None,
    ) -> QueryResponse:
        """Ask a question and get results with source attribution.

        Args:
            question: Natural language query.
            doc_ids: Limit query to specific document IDs.
            workspace_scope: Query across all indexed documents.
            timeout_secs: Per-operation timeout.
        """
        # Emit start event
        self._events.emit_query(
            QueryEventData(
                event_type=QueryEventType.STARTED,
                query=question,
            )
        )

        ctx = QueryContext(question)
        if doc_ids is not None:
            ctx = ctx.with_doc_ids(doc_ids)
        elif workspace_scope:
            ctx = ctx.with_workspace()
        if timeout_secs is not None:
            ctx = ctx.with_timeout_secs(timeout_secs)

        result = await self._engine.query(ctx)
        response = QueryResponse.from_rust(result)

        # Emit complete event
        self._events.emit_query(
            QueryEventData(
                event_type=QueryEventType.COMPLETE,
                query=question,
                total_results=len(response.items),
            )
        )

        return response

    async def query_stream(
        self,
        question: str,
        **kwargs: Any,
    ) -> StreamingQueryResult:
        """Stream query progress as an async iterator.

        Note: Currently wraps ``ask()`` and yields synthetic events.
        Real-time streaming requires Rust-side ``query_stream()`` exposure.
        """
        response = await self.ask(question, **kwargs)
        return StreamingQueryResult(response)

    # ── Document Management ───────────────────────────────────

    async def list_documents(self) -> list:
        """List all indexed documents."""
        return await self._engine.list()

    async def remove_document(self, doc_id: str) -> bool:
        """Remove a document by ID."""
        return await self._engine.remove(doc_id)

    async def document_exists(self, doc_id: str) -> bool:
        """Check if a document exists."""
        return await self._engine.exists(doc_id)

    async def clear_all(self) -> int:
        """Remove all indexed documents. Returns count removed."""
        return await self._engine.clear()

    # ── Graph ─────────────────────────────────────────────────

    async def get_graph(self) -> Optional[DocumentGraphWrapper]:
        """Get the cross-document relationship graph."""
        graph = await self._engine.get_graph()
        if graph is None:
            return None
        return DocumentGraphWrapper.from_rust(graph)

    # ── Metrics ───────────────────────────────────────────────

    def metrics_report(self) -> Any:
        """Get a comprehensive metrics report."""
        return self._engine.metrics_report()

    # ── Context Manager ───────────────────────────────────────

    async def __aenter__(self) -> "Session":
        return self

    async def __aexit__(self, *args: Any) -> None:
        pass

    def __repr__(self) -> str:
        model = self._config.llm.model or "unknown"
        return f"Session(model={model!r})"
