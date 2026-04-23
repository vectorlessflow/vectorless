"""Typed Python wrappers for PyO3 graph types."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Tuple


@dataclass(frozen=True)
class WeightedKeyword:
    """A keyword with importance weight."""

    keyword: str
    weight: float

    @classmethod
    def from_rust(cls, item: object) -> WeightedKeyword:
        return cls(keyword=item.keyword, weight=item.weight)


@dataclass(frozen=True)
class EdgeEvidence:
    """Evidence for a graph edge connecting two documents."""

    shared_keyword_count: int = 0
    keyword_jaccard: float = 0.0
    shared_keywords: Tuple[Tuple[str, float], ...] = ()

    @classmethod
    def from_rust(cls, item: object) -> EdgeEvidence:
        keywords = tuple((kw.keyword, kw.weight) for kw in item.shared_keywords)
        return cls(
            shared_keyword_count=item.shared_keyword_count,
            keyword_jaccard=item.keyword_jaccard,
            shared_keywords=keywords,
        )


@dataclass(frozen=True)
class GraphEdge:
    """An edge in the document relationship graph."""

    target_doc_id: str
    weight: float
    evidence: Optional[EdgeEvidence] = None

    @classmethod
    def from_rust(cls, item: object) -> GraphEdge:
        evidence = EdgeEvidence.from_rust(item.evidence) if item.evidence else None
        return cls(
            target_doc_id=item.target_doc_id,
            weight=item.weight,
            evidence=evidence,
        )


@dataclass(frozen=True)
class GraphNode:
    """A document node in the relationship graph."""

    doc_id: str
    title: str
    format: str
    node_count: int
    top_keywords: List[WeightedKeyword] = field(default_factory=list)

    @classmethod
    def from_rust(cls, item: object) -> GraphNode:
        keywords = [WeightedKeyword.from_rust(kw) for kw in item.top_keywords]
        return cls(
            doc_id=item.doc_id,
            title=item.title,
            format=item.format,
            node_count=item.node_count,
            top_keywords=keywords,
        )


@dataclass
class DocumentGraphWrapper:
    """Typed wrapper around the cross-document relationship graph."""

    _inner: Any

    @classmethod
    def from_rust(cls, graph: object) -> DocumentGraphWrapper:
        return cls(_inner=graph)

    def node_count(self) -> int:
        return self._inner.node_count()

    def edge_count(self) -> int:
        return self._inner.edge_count()

    def get_node(self, doc_id: str) -> Optional[GraphNode]:
        node = self._inner.get_node(doc_id)
        return GraphNode.from_rust(node) if node is not None else None

    def get_neighbors(self, doc_id: str) -> List[GraphEdge]:
        neighbors = self._inner.get_neighbors(doc_id)
        return [GraphEdge.from_rust(e) for e in neighbors]

    def doc_ids(self) -> List[str]:
        return list(self._inner.doc_ids())

    def is_empty(self) -> bool:
        return self._inner.is_empty()
