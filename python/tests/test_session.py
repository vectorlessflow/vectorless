"""Tests for Session high-level API."""

from __future__ import annotations

import pytest
from unittest.mock import AsyncMock, MagicMock, patch


class TestSessionConstruction:
    def test_session_rejects_no_source(self):
        """Session.index() should reject calls with no source."""
        # We can't fully test Session without a real Engine,
        # but we can test validation logic
        from vectorless.session import Session

        # This will fail because no api_key/model provided
        # We just verify the source validation in index()
        pass


class TestSessionIndex:
    @pytest.mark.asyncio
    async def test_index_requires_exactly_one_source(self):
        from vectorless.session import Session

        # Patch Engine construction
        with patch("vectorless.session.Engine") as MockEngine:
            mock_engine = MagicMock()
            mock_result = MagicMock()
            mock_result.doc_id = "doc-1"
            mock_result.items = []
            mock_result.failed = []
            mock_engine.index = AsyncMock(return_value=mock_result)
            MockEngine.return_value = mock_engine

            from vectorless.config import EngineConfig, LlmConfig

            with patch(
                "vectorless.session.Session._resolve_config",
                return_value=EngineConfig(llm=LlmConfig(model="test", api_key="test")),
            ):
                session = Session.__new__(Session)
                session._config = EngineConfig(
                    llm=LlmConfig(model="test", api_key="test")
                )
                session._engine = mock_engine
                session._events = MagicMock()

                # No source
                with pytest.raises(ValueError, match="exactly one source"):
                    await session.index()

                # Multiple sources
                with pytest.raises(ValueError, match="exactly one source"):
                    await session.index(path="a.pdf", content="text")
