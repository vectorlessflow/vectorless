"""High-level Vectorless Engine API.

``Engine`` is the single recommended entry point for all operations.
It wraps the Rust compile layer with Python strategy for retrieval:
typed configuration, event callbacks, flexible input methods, and batch operations.
"""

from __future__ import annotations

import asyncio
import logging
from pathlib import Path
from typing import Any, List, Optional, Union

from vectorless._core import Engine as RustEngine
from vectorless._core import IndexContext, IndexOptions
from vectorless.agent.orchestrator import DocCard, Orchestrator, OrchestratorResult
from vectorless.config import EngineConfig, load_config, load_config_from_env, load_config_from_file
from vectorless.events import (
    EventEmitter,
    IndexEventData,
    IndexEventType,
    QueryEventData,
    QueryEventType,
)
from vectorless.llm_client import LLMClient
from vectorless.query.plan import QueryIntent
from vectorless.query.understand import understand
from vectorless.rerank.synthesize import RerankOutput, process
from vectorless.streaming import StreamingQueryResult
from vectorless.types.graph import DocumentGraphWrapper
from vectorless.types.results import (
    Evidence,
    FailedItem,
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

        result = await self._rust.index(ctx)

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
        # Emit start event
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

        # Emit complete event
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
    ) -> QueryResponse:
        """Run the full Python strategy: understand → orchestrator → rerank."""
        # 1. Resolve target documents
        if doc_ids is not None:
            target_ids = doc_ids
        else:
            all_docs = await self._rust.list()
            target_ids = [d.doc_id for d in all_docs]

        if not target_ids:
            return QueryResponse(items=[], failed=[])

        # 2. Build DocCards for orchestrator analysis
        all_doc_infos = await self._rust.list()
        info_map = {d.doc_id: d for d in all_doc_infos}
        target_infos = [info_map[did] for did in target_ids if did in info_map]

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
        query_plan = await understand(question, self._llm)

        # 4. Orchestrator
        orchestrator = Orchestrator(
            query=question,
            doc_cards=doc_cards,
            doc_loader=self._load_document,
            llm_client=self._llm,
            skip_analysis=not workspace_scope and len(doc_cards) <= 1,
            intent_context=query_plan.intent_context(),
        )
        orch_result = await orchestrator.run()

        # 5. Rerank + synthesize
        reranked = process(
            evidence=orch_result.evidence,
            intent=query_plan.intent,
            confidence=orch_result.confidence,
        )

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
        return await self._rust.list()

    async def remove_document(self, doc_id: str) -> bool:
        """Remove a document by ID."""
        return await self._rust.remove(doc_id)

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

    # Build a single QueryResult from all evidence
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
