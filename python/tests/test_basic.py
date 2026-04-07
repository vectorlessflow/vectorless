# Copyright (c) 2026 vectorless developers
# SPDX-License-Identifier: Apache-2.0

"""Basic tests for vectorless Python bindings."""

import pytest


def test_import():
    """Test that we can import the module."""
    import vectorless

    assert hasattr(vectorless, "Engine")
    assert hasattr(vectorless, "IndexContext")
    assert hasattr(vectorless, "QueryResult")
    assert hasattr(vectorless, "DocumentInfo")
    assert hasattr(vectorless, "VectorlessError")


def test_version():
    """Test that version is available."""
    import vectorless

    assert vectorless.__version__ is not None
    assert isinstance(vectorless.__version__, str)


def test_index_context_from_file():
    """Test creating IndexContext from file."""
    from vectorless import IndexContext

    ctx = IndexContext.from_file("./test.md")
    assert ctx is not None


def test_index_context_from_file_with_name():
    """Test creating IndexContext from file with custom name."""
    from vectorless import IndexContext

    ctx = IndexContext.from_file("./test.md", name="custom_name")
    assert ctx is not None


def test_index_context_from_text():
    """Test creating IndexContext from text."""
    from vectorless import IndexContext

    ctx = IndexContext.from_text("# Test\n\nContent here.")
    assert ctx is not None


def test_index_context_from_text_with_name():
    """Test creating IndexContext from text with custom name."""
    from vectorless import IndexContext

    ctx = IndexContext.from_text(
        "# Test\n\nContent here.",
        name="test_doc",
        format="markdown",
    )
    assert ctx is not None


def test_index_context_from_text_html():
    """Test creating IndexContext from HTML text."""
    from vectorless import IndexContext

    ctx = IndexContext.from_text(
        "<html><body><h1>Title</h1><p>Content</p></body></html>",
        name="page",
        format="html",
    )
    assert ctx is not None


def test_index_context_from_bytes():
    """Test creating IndexContext from bytes."""
    from vectorless import IndexContext

    data = b"%PDF-1.4\n%fake pdf"
    ctx = IndexContext.from_bytes(data, name="test.pdf", format="pdf")
    assert ctx is not None


def test_index_context_invalid_format():
    """Test that invalid format raises error."""
    from vectorless import IndexContext, VectorlessError

    with pytest.raises(VectorlessError) as exc_info:
        IndexContext.from_text("content", format="invalid_format")

    assert "Unknown format" in str(exc_info.value.message)
    assert exc_info.value.kind == "config"


@pytest.mark.asyncio
async def test_engine_create():
    """Test creating an engine."""
    import tempfile
    from vectorless import Engine

    with tempfile.TemporaryDirectory() as tmpdir:
        engine = Engine(workspace=tmpdir)
        assert engine is not None


@pytest.mark.asyncio
async def test_engine_len():
    """Test engine document count."""
    import tempfile
    from vectorless import Engine

    with tempfile.TemporaryDirectory() as tmpdir:
        engine = Engine(workspace=tmpdir)
        count = engine.len()
        assert isinstance(count, int)
        assert count >= 0


@pytest.mark.asyncio
async def test_engine_list_docs():
    """Test listing documents."""
    import tempfile
    from vectorless import Engine

    with tempfile.TemporaryDirectory() as tmpdir:
        engine = Engine(workspace=tmpdir)
        docs = engine.list_docs()
        assert isinstance(docs, list)


@pytest.mark.asyncio
async def test_engine_clear():
    """Test clearing all documents."""
    import tempfile
    from vectorless import Engine

    with tempfile.TemporaryDirectory() as tmpdir:
        engine = Engine(workspace=tmpdir)
        removed = engine.clear()
        assert isinstance(removed, int)


@pytest.mark.asyncio
async def test_engine_exists():
    """Test checking if document exists."""
    import tempfile
    from vectorless import Engine

    with tempfile.TemporaryDirectory() as tmpdir:
        engine = Engine(workspace=tmpdir)
        exists = engine.exists("nonexistent")
        assert exists is False


@pytest.mark.asyncio
async def test_index_and_query_text():
    """Test indexing and querying a text document."""
    import tempfile
    from vectorless import Engine, IndexContext

    with tempfile.TemporaryDirectory() as tmpdir:
        engine = Engine(workspace=tmpdir)

        # Index a simple document
        ctx = IndexContext.from_text(
            "# Test Document\n\nThis is a test document about apples.",
            name="test",
        )
        doc_id = engine.index(ctx)

        assert doc_id is not None
        assert isinstance(doc_id, str)

        # Query the document
        result = engine.query(doc_id, "What is this document about?")

        assert result.doc_id == doc_id
        assert result.content is not None
        assert result.score >= 0.0


@pytest.mark.asyncio
async def test_remove_document():
    """Test removing a document."""
    import tempfile
    from vectorless import Engine, IndexContext

    with tempfile.TemporaryDirectory() as tmpdir:
        engine = Engine(workspace=tmpdir)

        # Index a document
        ctx = IndexContext.from_text("# Test\n\nContent", name="test")
        doc_id = engine.index(ctx)

        # Remove it
        removed = engine.remove(doc_id)
        assert removed is True

        # Check it's gone
        exists = engine.exists(doc_id)
        assert exists is False


@pytest.mark.asyncio
async def test_query_nonexistent():
    """Test querying a nonexistent document."""
    import tempfile
    from vectorless import Engine, VectorlessError

    with tempfile.TemporaryDirectory() as tmpdir:
        engine = Engine(workspace=tmpdir)

        with pytest.raises(VectorlessError) as exc_info:
            engine.query("nonexistent", "question")

        assert exc_info.value.kind == "not_found"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
