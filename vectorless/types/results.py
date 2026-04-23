"""Typed Python wrappers for PyO3 result types."""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Iterator, List, Optional


@dataclass(frozen=True)
class Evidence:
    """A single piece of evidence with source attribution."""

    title: str
    path: str
    content: str
    doc_name: Optional[str] = None

    @classmethod
    def from_rust(cls, item: object) -> Evidence:
        return cls(
            title=item.title,
            path=item.path,
            content=item.content,
            doc_name=item.doc_name,
        )

    def to_dict(self) -> dict:
        d = {"title": self.title, "path": self.path, "content": self.content}
        if self.doc_name is not None:
            d["doc_name"] = self.doc_name
        return d

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), ensure_ascii=False)


@dataclass(frozen=True)
class QueryMetrics:
    """Metrics from a single query execution."""

    llm_calls: int = 0
    rounds_used: int = 0
    nodes_visited: int = 0
    evidence_count: int = 0
    evidence_chars: int = 0

    @classmethod
    def from_rust(cls, item: object) -> QueryMetrics:
        return cls(
            llm_calls=item.llm_calls,
            rounds_used=item.rounds_used,
            nodes_visited=item.nodes_visited,
            evidence_count=item.evidence_count,
            evidence_chars=item.evidence_chars,
        )

    def to_dict(self) -> dict:
        return {
            "llm_calls": self.llm_calls,
            "rounds_used": self.rounds_used,
            "nodes_visited": self.nodes_visited,
            "evidence_count": self.evidence_count,
            "evidence_chars": self.evidence_chars,
        }


@dataclass(frozen=True)
class QueryResult:
    """A single document's query result."""

    doc_id: str
    content: str
    score: float
    confidence: float
    node_ids: List[str] = field(default_factory=list)
    evidence: List[Evidence] = field(default_factory=list)
    metrics: Optional[QueryMetrics] = None

    @classmethod
    def from_rust(cls, item: object) -> QueryResult:
        evidence = [Evidence.from_rust(e) for e in item.evidence]
        metrics = QueryMetrics.from_rust(item.metrics) if item.metrics else None
        return cls(
            doc_id=item.doc_id,
            content=item.content,
            score=item.score,
            confidence=item.confidence,
            node_ids=list(item.node_ids),
            evidence=evidence,
            metrics=metrics,
        )

    def to_dict(self) -> dict:
        d = {
            "doc_id": self.doc_id,
            "content": self.content,
            "score": self.score,
            "confidence": self.confidence,
            "node_ids": self.node_ids,
            "evidence": [e.to_dict() for e in self.evidence],
        }
        if self.metrics:
            d["metrics"] = self.metrics.to_dict()
        return d


@dataclass(frozen=True)
class FailedItem:
    """A failed item in a batch operation."""

    source: str
    error: str

    @classmethod
    def from_rust(cls, item: object) -> FailedItem:
        return cls(source=item.source, error=item.error)


@dataclass(frozen=True)
class QueryResponse:
    """Wraps a complete query result (potentially multi-document)."""

    items: List[QueryResult] = field(default_factory=list)
    failed: List[FailedItem] = field(default_factory=list)

    @classmethod
    def from_rust(cls, result: object) -> QueryResponse:
        items = [QueryResult.from_rust(i) for i in result.items]
        failed = [FailedItem.from_rust(f) for f in result.failed]
        return cls(items=items, failed=failed)

    def single(self) -> Optional[QueryResult]:
        """Get the first (single-doc) result item."""
        return self.items[0] if self.items else None

    def has_failures(self) -> bool:
        return len(self.failed) > 0

    def __len__(self) -> int:
        return len(self.items)

    def __iter__(self) -> Iterator[QueryResult]:  # type: ignore[override]
        return iter(self.items)

    def to_dict(self) -> dict:
        return {
            "items": [i.to_dict() for i in self.items],
            "failed": [{"source": f.source, "error": f.error} for f in self.failed],
        }


@dataclass(frozen=True)
class IndexMetrics:
    """Metrics from the indexing pipeline."""

    total_time_ms: int = 0
    parse_time_ms: int = 0
    build_time_ms: int = 0
    enhance_time_ms: int = 0
    nodes_processed: int = 0
    summaries_generated: int = 0
    summaries_failed: int = 0
    llm_calls: int = 0
    total_tokens_generated: int = 0
    topics_indexed: int = 0
    keywords_indexed: int = 0

    @classmethod
    def from_rust(cls, item: object) -> IndexMetrics:
        return cls(
            total_time_ms=item.total_time_ms,
            parse_time_ms=item.parse_time_ms,
            build_time_ms=item.build_time_ms,
            enhance_time_ms=item.enhance_time_ms,
            nodes_processed=item.nodes_processed,
            summaries_generated=item.summaries_generated,
            summaries_failed=item.summaries_failed,
            llm_calls=item.llm_calls,
            total_tokens_generated=item.total_tokens_generated,
            topics_indexed=item.topics_indexed,
            keywords_indexed=item.keywords_indexed,
        )


@dataclass(frozen=True)
class IndexItemWrapper:
    """A single indexed document item."""

    doc_id: str
    name: str
    format: str
    description: Optional[str] = None
    source_path: Optional[str] = None
    page_count: Optional[int] = None
    metrics: Optional[IndexMetrics] = None

    @classmethod
    def from_rust(cls, item: object) -> IndexItemWrapper:
        metrics = IndexMetrics.from_rust(item.metrics) if item.metrics else None
        return cls(
            doc_id=item.doc_id,
            name=item.name,
            format=item.format,
            description=item.description,
            source_path=item.source_path,
            page_count=item.page_count,
            metrics=metrics,
        )


@dataclass(frozen=True)
class IndexResultWrapper:
    """Result of a document indexing operation."""

    doc_id: Optional[str] = None
    items: List[IndexItemWrapper] = field(default_factory=list)
    failed: List[FailedItem] = field(default_factory=list)

    @classmethod
    def from_doc_info(cls, doc_info: object) -> IndexResultWrapper:
        """Create from a single Rust PyDocumentInfo (returned by ingest)."""
        item = IndexItemWrapper(
            doc_id=doc_info.doc_id,
            name=doc_info.name,
            format=doc_info.format,
            description=getattr(doc_info, "description", None),
            source_path=getattr(doc_info, "source_path", None),
            page_count=getattr(doc_info, "page_count", None),
        )
        return cls(doc_id=doc_info.doc_id, items=[item])

    @classmethod
    def from_doc_infos(cls, doc_infos: list) -> IndexResultWrapper:
        """Create from a list of Rust PyDocumentInfo objects."""
        items = []
        first_doc_id = None
        for info in doc_infos:
            if first_doc_id is None:
                first_doc_id = info.doc_id
            items.append(IndexItemWrapper(
                doc_id=info.doc_id,
                name=info.name,
                format=info.format,
                description=getattr(info, "description", None),
                source_path=getattr(info, "source_path", None),
                page_count=getattr(info, "page_count", None),
            ))
        return cls(doc_id=first_doc_id, items=items)

    @classmethod
    def from_rust(cls, result: object) -> IndexResultWrapper:
        items = [IndexItemWrapper.from_rust(i) for i in result.items]
        failed = [FailedItem.from_rust(f) for f in result.failed]
        return cls(
            doc_id=result.doc_id,
            items=items,
            failed=failed,
        )

    def has_failures(self) -> bool:
        return len(self.failed) > 0

    def total(self) -> int:
        return len(self.items) + len(self.failed)

    def __len__(self) -> int:
        return len(self.items)
