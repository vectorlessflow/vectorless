"""LlamaIndex retriever integration for Vectorless."""

from __future__ import annotations

from typing import Any, List, Optional

from vectorless._async_utils import run_async
from vectorless.engine import Engine


class VectorlessRetriever:
    """LlamaIndex-compatible retriever backed by Vectorless.

    Usage::

        from vectorless._compat import get_llamaindex_retriever

        VectorlessRetriever = get_llamaindex_retriever()

        retriever = VectorlessRetriever(
            api_key="sk-...",
            model="gpt-4o",
            endpoint="https://api.openai.com/v1",
            doc_ids=["doc-123"],
        )

        nodes = retriever.retrieve("What is the revenue?")
    """

    def __init__(
        self,
        api_key: str = "",
        model: str = "",
        endpoint: str = "",
        doc_ids: Optional[List[str]] = None,
        top_k: int = 3,
        workspace_scope: bool = False,
        engine: Optional[Engine] = None,
    ) -> None:
        if engine is not None:
            self._engine = engine
        else:
            self._engine = Engine(
                api_key=api_key or None,
                model=model or None,
                endpoint=endpoint or None,
            )
        self._doc_ids = doc_ids or []
        self._top_k = top_k
        self._workspace_scope = workspace_scope

    def retrieve(self, query: str) -> List[Any]:
        """Synchronous retrieval, returns LlamaIndex NodeWithScore objects."""
        response = run_async(self._query(query))
        return self._to_nodes(response)

    async def aretrieve(self, query: str) -> List[Any]:
        """Async retrieval, returns LlamaIndex NodeWithScore objects."""
        response = await self._query(query)
        return self._to_nodes(response)

    async def _query(self, query: str) -> Any:
        return await self._engine.ask(
            query,
            doc_ids=self._doc_ids if self._doc_ids else None,
            workspace_scope=self._workspace_scope,
        )

    @staticmethod
    def _to_nodes(response: Any) -> List[Any]:
        """Convert Vectorless QueryResponse to LlamaIndex NodeWithScore."""
        from llama_index.core.schema import NodeWithScore, TextNode

        nodes = []
        for item in response.items:
            metadata = {
                "doc_id": item.doc_id,
                "confidence": item.confidence,
                "node_ids": item.node_ids,
            }
            text_node = TextNode(
                text=item.content,
                metadata=metadata,
            )
            nodes.append(
                NodeWithScore(
                    node=text_node,
                    score=item.score,
                )
            )
        return nodes
