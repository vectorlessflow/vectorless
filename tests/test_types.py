"""Tests for typed result wrappers."""

from __future__ import annotations

from unittest.mock import MagicMock

from vectorless.types.results import (
    Evidence,
    FailedItem,
    IndexItemWrapper,
    IndexMetrics,
    IndexResultWrapper,
    QueryMetrics,
    QueryResponse,
    QueryResult,
)


class TestEvidence:
    def test_from_rust(self):
        item = MagicMock()
        item.title = "Section 1"
        item.path = "Root/Section 1"
        item.content = "Some evidence text"
        item.doc_name = "report.pdf"

        ev = Evidence.from_rust(item)
        assert ev.title == "Section 1"
        assert ev.path == "Root/Section 1"
        assert ev.content == "Some evidence text"
        assert ev.doc_name == "report.pdf"

    def test_to_dict(self):
        ev = Evidence(title="T", path="P", content="C", doc_name=None)
        d = ev.to_dict()
        assert d == {"title": "T", "path": "P", "content": "C"}

    def test_to_json(self):
        ev = Evidence(title="T", path="P", content="C")
        import json

        parsed = json.loads(ev.to_json())
        assert parsed["title"] == "T"

    def test_frozen(self):
        ev = Evidence(title="T", path="P", content="C")
        with pytest.raises(AttributeError):
            ev.title = "new"


class TestQueryResult:
    def test_from_rust(self):
        item = MagicMock()
        item.doc_id = "doc-1"
        item.content = "Result text"
        item.score = 0.9
        item.confidence = 0.9
        item.node_ids = ["node-1", "node-2"]
        item.evidence = []
        item.metrics = None

        result = QueryResult.from_rust(item)
        assert result.doc_id == "doc-1"
        assert result.content == "Result text"
        assert result.score == 0.9
        assert len(result.node_ids) == 2
        assert result.metrics is None

    def test_to_dict(self):
        result = QueryResult(
            doc_id="doc-1",
            content="text",
            score=0.9,
            confidence=0.9,
            node_ids=["n1"],
            evidence=[],
            metrics=None,
        )
        d = result.to_dict()
        assert d["doc_id"] == "doc-1"
        assert "metrics" not in d


class TestQueryResponse:
    def test_from_rust(self):
        rust_result = MagicMock()
        rust_result.items = []
        rust_result.failed = []

        response = QueryResponse.from_rust(rust_result)
        assert len(response) == 0
        assert response.single() is None
        assert not response.has_failures()

    def test_single(self):
        item = QueryResult(
            doc_id="doc-1", content="text", score=0.9, confidence=0.9
        )
        response = QueryResponse(items=[item])
        assert response.single() == item
        assert len(response) == 1

    def test_iteration(self):
        items = [
            QueryResult(doc_id=f"doc-{i}", content="t", score=0.5, confidence=0.5)
            for i in range(3)
        ]
        response = QueryResponse(items=items)
        assert list(response) == items

    def test_to_dict(self):
        response = QueryResponse(
            items=[QueryResult(doc_id="d", content="t", score=0.5, confidence=0.5)],
            failed=[FailedItem(source="s", error="e")],
        )
        d = response.to_dict()
        assert len(d["items"]) == 1
        assert len(d["failed"]) == 1
        assert d["failed"][0]["source"] == "s"


class TestIndexResult:
    def test_from_rust(self):
        rust_result = MagicMock()
        rust_result.doc_id = "doc-1"
        item = MagicMock()
        item.doc_id = "doc-1"
        item.name = "test.md"
        item.format = "markdown"
        item.description = None
        item.source_path = None
        item.page_count = None
        item.metrics = None
        rust_result.items = [item]
        rust_result.failed = []

        result = IndexResultWrapper.from_rust(rust_result)
        assert result.doc_id == "doc-1"
        assert len(result.items) == 1
        assert result.items[0].name == "test.md"
        assert not result.has_failures()
        assert result.total() == 1


import pytest
