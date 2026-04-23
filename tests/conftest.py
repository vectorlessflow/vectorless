"""Shared test fixtures."""

from __future__ import annotations

import pytest
from unittest.mock import AsyncMock, MagicMock


@pytest.fixture
def mock_engine():
    """Mock Rust Engine for testing without LLM."""
    engine = MagicMock()

    # Mock index result
    index_result = MagicMock()
    index_result.doc_id = "test-doc-id"
    index_item = MagicMock()
    index_item.doc_id = "test-doc-id"
    index_item.name = "test.md"
    index_item.format = "markdown"
    index_item.description = None
    index_item.source_path = "/path/to/test.md"
    index_item.page_count = None
    index_item.metrics = None
    index_result.items = [index_item]
    index_result.failed = []
    index_result.has_failures.return_value = False
    index_result.total.return_value = 1
    index_result.__len__ = lambda self: 1

    engine.index = AsyncMock(return_value=index_result)

    # Mock query result
    query_item = MagicMock()
    query_item.doc_id = "test-doc-id"
    query_item.content = "Test answer content"
    query_item.score = 0.85
    query_item.confidence = 0.85
    query_item.node_ids = ["node-1"]
    query_item.evidence = []
    query_item.metrics = None

    query_result = MagicMock()
    query_result.items = [query_item]
    query_result.failed = []
    query_result.single.return_value = query_item
    query_result.has_failures.return_value = False
    query_result.__len__ = lambda self: 1

    engine.query = AsyncMock(return_value=query_result)

    # Mock list
    doc_info = MagicMock()
    doc_info.id = "test-doc-id"
    doc_info.name = "test.md"
    doc_info.format = "markdown"
    doc_info.description = None
    doc_info.source_path = "/path/to/test.md"
    doc_info.page_count = None
    doc_info.line_count = 42
    engine.list = AsyncMock(return_value=[doc_info])

    # Mock other operations
    engine.remove = AsyncMock(return_value=True)
    engine.clear = AsyncMock(return_value=1)
    engine.exists = AsyncMock(return_value=True)

    # Mock graph
    engine.get_graph = AsyncMock(return_value=None)

    # Mock metrics
    metrics_report = MagicMock()
    metrics_report.total_cost_usd.return_value = 0.001
    engine.metrics_report.return_value = metrics_report

    return engine


@pytest.fixture
def sample_config_dict():
    """Sample configuration dict."""
    return {
        "llm": {
            "model": "gpt-4o",
            "api_key": "sk-test-key",
            "endpoint": "https://api.openai.com/v1",
        },
        "retrieval": {"top_k": 5},
        "storage": {"workspace_dir": "/tmp/test-vectorless"},
    }
