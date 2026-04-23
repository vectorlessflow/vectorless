"""High-level Vectorless Engine API.

``Engine`` is the single recommended entry point for all operations.
It wraps the Rust compile layer with Python strategy for retrieval:
typed configuration, event callbacks, flexible input methods, and batch operations.
"""

from __future__ import annotations

import asyncio
import logging
from pathlib import Path
from typing import Any, Callable, List, Optional, Union

from vectorless._internal._core import Engine as RustEngine
from vectorless.ask.orchestrator import DocCard, Orchestrator, OrchestratorResult
from vectorless.config import EngineConfig, load_config, load_config_from_env, load_config_from_file
from vectorless.events import (
    EventEmitter,
    IndexEventData,
    IndexEventType,
    QueryEventData,
    QueryEventType,
)
from vectorless.llm_client import LLMClient
from vectorless.ask.plan import QueryIntent
from vectorless.ask.understand import understand
from vectorless.rerank.synthesize import RerankOutput, process
from vectorless.streaming import StreamingQueryResult
from vectorless.types.graph import DocumentGraphWrapper
from vectorless.types.results import (
    Evidence,
    IndexResultWrapper,
    QueryMetrics,
    QueryResponse,
    QueryResult,
)

logger = logging.getLogger(__name__)


class Engine:
    """High-level Vectorless engine.

    compile (ingest) runs in Rust; ask (retrieval) runs in Python.

    Configuration precedence: constructor args > env vars > config file > defaults.

    Usage::

        from vectorless import Engine

        engine = Engine(api_key="sk-...", model="gpt-4o")
        result = await engine.index(path="./report.pdf")
        answer = await engine.ask("What is the Q4 revenue?", doc_ids=[result.doc_id])
        print(answer.single().content)

    Or from environment variables::

        # VECTORLESS_API_KEY, VECTORLESS_MODEL set in env
        engine = Engine.from_env()
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

        # Build Rust engine (for compile / document management)
        rust_config = self._config.to_rust_config()
        self._rust = RustEngine(
            api_key=self._config.llm.api_key,
            model=self._config.llm.model or None,
            endpoint=self._config.llm.endpoint or None,
            config=rust_config,
        )

        # Build Python LLM client (for strategy layer)
        self._llm = LLMClient(
            api_key=self._config.llm.api_key,
            model=self._config.llm.model,
            endpoint=self._config.llm.endpoint or None,
        )

    @classmethod
    def from_env(cls, events: Optional[EventEmitter] = None) -> Engine:
        """Create an Engine from environment variables only."""
        config = load_config_from_env()
        return cls(config=config, events=events)

    @classmethod
    def from_config_file(
        cls,
        path: Union[str, Path],
        events: Optional[EventEmitter] = None,
    ) -> Engine:
        """Create an Engine from a TOML config file."""
        config = load_config_from_file(Path(path))
        return cls(config=config, events=events)

    def _resolve_config(
        self,
        api_key: Optional[str],
        model: Optional[str],
        endpoint: Optional[str],
        config_file: Optional[Union[str, Path]],
    ) -> EngineConfig:
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

    # ── Indexing (Rust compile pipeline) ────────────────────────

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

        # For single file, delegate to Rust ingest
        if path is not None:
            source_desc = str(path)
            self._events.emit_index(
                IndexEventData(event_type=IndexEventType.STARTED, path=source_desc)
            )
            doc_info = await self._rust.ingest(str(path))
            self._events.emit_index(
                IndexEventData(
                    event_type=IndexEventType.COMPLETE,
                    doc_id=doc_info.doc_id,
                    message=f"Indexed {doc_info.doc_id}",
                )
            )
            return IndexResultWrapper.from_doc_info(doc_info)

        # For multiple files, index them sequentially
        if paths is not None:
            return await self.index_batch(
                paths, mode="force" if force else mode,
            )

        if directory is not None:
            # Scan directory for supported files
            dir_path = Path(directory)
            extensions = {".md", ".pdf", ".markdown"}
            file_paths = [
                str(f) for f in dir_path.rglob("*")
                if f.suffix.lower() in extensions and f.is_file()
            ]
            if not file_paths:
                raise ValueError(f"No supported documents found in {directory}")
            return await self.index_batch(file_paths, mode="force" if force else mode)

        if content is not None:
            # Write content to a temp file and ingest
            import tempfile
            suffix = ".md" if format == "markdown" else f".{format}"
            with tempfile.NamedTemporaryFile(mode="w", suffix=suffix, delete=False) as f:
                f.write(content)
                tmp_path = f.name
            try:
                doc_info = await self._rust.ingest(tmp_path)
                return IndexResultWrapper.from_doc_info(doc_info)
            finally:
                import os
                os.unlink(tmp_path)

        if bytes_data is not None:
            import tempfile
            suffix = ".md" if format == "markdown" else f".{format}"
            with tempfile.NamedTemporaryFile(mode="wb", suffix=suffix, delete=False) as f:
                f.write(bytes_data)
                tmp_path = f.name
            try:
                doc_info = await self._rust.ingest(tmp_path)
                return IndexResultWrapper.from_doc_info(doc_info)
            finally:
                import os
                os.unlink(tmp_path)

        raise ValueError("No source provided")

    async def index_batch(
        self,
        paths: List[Union[str, Path]],
        *,
        mode: str = "default",
        jobs: int = 1,
        force: bool = False,
        progress: bool = True,
    ) -> IndexResultWrapper:
        """Index multiple files with optional concurrency.

        Args:
            paths: List of file paths to index.
            mode: Indexing mode ("default", "force", "incremental").
            jobs: Max concurrent indexing jobs.
            force: Force re-index existing documents.
            progress: Emit progress events.
        """
        semaphore = asyncio.Semaphore(jobs)

        async def _index_one(p: Union[str, Path]) -> object:
            async with semaphore:
                self._events.emit_index(
                    IndexEventData(event_type=IndexEventType.STARTED, path=str(p))
                )
                doc_info = await self._rust.ingest(str(p))
                if progress:
                    self._events.emit_index(
                        IndexEventData(
                            event_type=IndexEventType.COMPLETE,
                            path=str(p),
                            doc_id=doc_info.doc_id,
                        )
                    )
                return doc_info

        results = await asyncio.gather(*[_index_one(p) for p in paths])
        return IndexResultWrapper.from_doc_infos(list(results))

    # ── Querying (Python strategy layer) ────────────────────────

    async def ask(
        self,
        question: str,
        *,
        doc_ids: Optional[List[str]] = None,
        workspace_scope: bool = False,
        timeout_secs: Optional[int] = None,
    ) -> QueryResponse:
        """Ask a question and get results with source attribution.

        Uses the Python strategy layer: query understanding → orchestrator → workers → rerank.

        Args:
            question: Natural language query.
            doc_ids: Limit query to specific document IDs.
            workspace_scope: Query across all indexed documents.
            timeout_secs: Per-operation timeout.
        """
        self._events.emit_query(
            QueryEventData(event_type=QueryEventType.STARTED, query=question)
        )

        try:
            result = await self._ask_python(question, doc_ids, workspace_scope)
        except Exception as e:
            self._events.emit_query(
                QueryEventData(
                    event_type=QueryEventType.ERROR,
                    query=question,
                    message=str(e),
                )
            )
            raise

        self._events.emit_query(
            QueryEventData(
                event_type=QueryEventType.COMPLETE,
                query=question,
                total_results=len(result.items),
            )
        )

        return result

    async def query_stream(
        self,
        question: str,
        *,
        doc_ids: Optional[List[str]] = None,
        workspace_scope: bool = False,
        timeout_secs: Optional[int] = None,
    ) -> StreamingQueryResult:
        """Stream query progress as an async iterator.

        Yields real-time events from the Python strategy pipeline.
        Terminal events are ``'completed'`` (with results) or ``'error'``.

        Usage::

            stream = await engine.query_stream("What is the revenue?")
            async for event in stream:
                print(event["type"], event)
            result = stream.result
        """
        return StreamingQueryResult.from_engine(self, question, doc_ids, workspace_scope)

    # ── Python strategy implementation ──────────────────────────

    async def _ask_python(
        self,
        question: str,
        doc_ids: Optional[List[str]],
        workspace_scope: bool,
        event_queue: Optional[asyncio.Queue] = None,
    ) -> QueryResponse:
        """Run the full Python strategy: understand → orchestrator → rerank.

        Args:
            event_queue: If provided, progress events are put into this queue
                         for streaming consumers.
        """
        emit = event_queue.put if event_queue else lambda _: asyncio.ensure_future(asyncio.sleep(0))

        # 1. Resolve target documents
        all_doc_infos = await self._rust.list_documents()

        if doc_ids is not None:
            target_ids = doc_ids
        else:
            target_ids = [d.doc_id for d in all_doc_infos]

        if not target_ids:
            return QueryResponse(items=[], failed=[])

        # 2. Build DocCards for orchestrator analysis
        info_map = {d.doc_id: d for d in all_doc_infos}
        target_infos = [info_map[did] for did in target_ids if did in info_map]

        if not target_infos:
            raise DocumentNotFoundError(
                f"None of the requested doc_ids found: {doc_ids}"
            )

        doc_cards = []
        for info in target_infos:
            concepts = []
            if info.concepts:
                concepts = [c.name for c in info.concepts]
            doc_cards.append(DocCard(
                doc_id=info.doc_id,
                name=info.name,
                summary=info.summary or "",
                section_count=info.section_count,
                concepts=concepts,
            ))

        # 3. Query understanding
        await emit({"type": "understanding_started", "query": question})
        query_plan = await understand(question, self._llm)
        await emit({
            "type": "understanding_done",
            "intent": query_plan.intent.value,
            "keywords": query_plan.keywords,
            "strategy_hint": query_plan.strategy_hint,
        })

        # 4. Orchestrator
        orchestrator = Orchestrator(
            query=question,
            doc_cards=doc_cards,
            doc_loader=self._load_document,
            llm_client=self._llm,
            skip_analysis=not workspace_scope and len(doc_cards) <= 1,
            intent_context=query_plan.intent_context(),
            event_callback=emit if event_queue else None,
        )

        await emit({
            "type": "orchestrator_started",
            "doc_count": len(doc_cards),
            "docs": [c.name for c in doc_cards],
        })
        orch_result = await orchestrator.run()
        await emit({
            "type": "orchestrator_done",
            "evidence_count": len(orch_result.evidence),
            "confidence": orch_result.confidence,
            "rounds_used": orch_result.rounds_used,
        })

        # 5. Rerank + synthesize
        await emit({"type": "rerank_started"})
        reranked = process(
            evidence=orch_result.evidence,
            intent=query_plan.intent,
            confidence=orch_result.confidence,
        )
        await emit({
            "type": "rerank_done",
            "evidence_count": len(reranked.evidence),
        })

        # 6. Convert to QueryResponse
        return _orchestrator_to_response(
            reranked, orch_result, target_ids, query_plan.intent,
        )

    async def _load_document(self, doc_id: str):
        """Load a navigable Document from the Rust engine."""
        return await self._rust.load_document(doc_id)

    # ── Document Management (Rust) ──────────────────────────────

    async def list_documents(self) -> list:
        """List all indexed documents."""
        return await self._rust.list_documents()

    async def remove_document(self, doc_id: str) -> bool:
        """Remove a document by ID."""
        await self._rust.forget(doc_id)
        return True

    async def document_exists(self, doc_id: str) -> bool:
        """Check if a document exists."""
        return await self._rust.exists(doc_id)

    async def clear_all(self) -> int:
        """Remove all indexed documents. Returns count removed."""
        return await self._rust.clear()

    # ── Graph (Rust) ────────────────────────────────────────────

    async def get_graph(self) -> Optional[DocumentGraphWrapper]:
        """Get the cross-document relationship graph."""
        graph = await self._rust.get_graph()
        if graph is None:
            return None
        return DocumentGraphWrapper.from_rust(graph)

    # ── Metrics (Rust) ──────────────────────────────────────────

    def metrics_report(self) -> Any:
        """Get a comprehensive metrics report."""
        return self._rust.metrics_report()

    # ── Context Manager ─────────────────────────────────────────

    async def __aenter__(self) -> Engine:
        return self

    async def __aexit__(self, *args: Any) -> None:
        pass

    def __repr__(self) -> str:
        model = self._config.llm.model or "unknown"
        return f"Engine(model={model!r})"


# ---------------------------------------------------------------------------
# Exceptions
# ---------------------------------------------------------------------------


class DocumentNotFoundError(Exception):
    """Raised when a requested document ID is not found in the workspace."""


class EmptyWorkspaceError(Exception):
    """Raised when no documents are indexed in the workspace."""


# ---------------------------------------------------------------------------
# Conversion helpers
# ---------------------------------------------------------------------------

def _orchestrator_to_response(
    reranked: RerankOutput,
    orch_result: OrchestratorResult,
    target_ids: list[str],
    intent: QueryIntent,
) -> QueryResponse:
    """Convert Python strategy output to QueryResponse."""
    if not reranked.evidence:
        return QueryResponse(items=[], failed=[])

    evidence_list = [
        Evidence(
            title=e.title,
            path=e.source_path,
            content=e.content,
        )
        for e in reranked.evidence
    ]

    metrics = QueryMetrics(
        llm_calls=orch_result.llm_calls,
        rounds_used=orch_result.rounds_used,
        nodes_visited=orch_result.nodes_visited,
        evidence_count=len(reranked.evidence),
        evidence_chars=sum(len(e.content) for e in reranked.evidence),
    )

    item = QueryResult(
        doc_id=target_ids[0] if len(target_ids) == 1 else "",
        content=reranked.answer,
        score=reranked.confidence,
        confidence=reranked.confidence,
        node_ids=[e.node_id for e in reranked.evidence],
        evidence=evidence_list,
        metrics=metrics,
    )

    return QueryResponse(items=[item], failed=[])
