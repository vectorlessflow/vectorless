"""LangChain BaseRetriever integration for Vectorless."""

from __future__ import annotations

from typing import Any, List, Optional

from langchain_core.callbacks import CallbackManagerForRetrieverRun
from langchain_core.documents import Document
from langchain_core.retrievers import BaseRetriever

from vectorless._async_utils import run_async
from vectorless.session import Session


class VectorlessRetriever(BaseRetriever):
    """LangChain retriever backed by Vectorless.

    Usage::

        from vectorless._compat import get_langchain_retriever

        VectorlessRetriever = get_langchain_retriever()

        retriever = VectorlessRetriever(
            api_key="sk-...",
            model="gpt-4o",
            endpoint="https://api.openai.com/v1",
            doc_ids=["doc-123"],
            top_k=3,
        )

        docs = retriever.invoke("What is the revenue?")

    Or with an existing Session (avoids re-initializing the engine)::

        from vectorless import Session

        session = Session(api_key="sk-...", model="gpt-4o")
        retriever = VectorlessRetriever(session=session, doc_ids=["doc-123"])
    """

    api_key: str = ""
    model: str = ""
    endpoint: str = ""
    doc_ids: List[str] = []
    top_k: int = 3
    workspace_scope: bool = False
    session: Optional[Session] = None

    class Config:
        arbitrary_types_allowed = True

    def _get_session(self) -> Session:
        """Get or lazily create a cached Session instance."""
        if self.session is None:
            self.session = Session(
                api_key=self.api_key or None,
                model=self.model or None,
                endpoint=self.endpoint or None,
            )
        return self.session

    def _get_relevant_documents(
        self,
        query: str,
        *,
        run_manager: Optional[CallbackManagerForRetrieverRun] = None,
    ) -> List[Document]:
        """Synchronous retrieval."""
        session = self._get_session()
        response = run_async(
            session.ask(
                query,
                doc_ids=self.doc_ids if self.doc_ids else None,
                workspace_scope=self.workspace_scope,
            )
        )
        return self._to_documents(response)

    async def _aget_relevant_documents(
        self,
        query: str,
        *,
        run_manager: Optional[CallbackManagerForRetrieverRun] = None,
    ) -> List[Document]:
        """Async retrieval."""
        session = self._get_session()
        response = await session.ask(
            query,
            doc_ids=self.doc_ids if self.doc_ids else None,
            workspace_scope=self.workspace_scope,
        )
        return self._to_documents(response)

    @staticmethod
    def _to_documents(response: Any) -> List[Document]:
        """Convert Vectorless QueryResponse to LangChain Documents."""
        documents = []
        for item in response.items:
            metadata = {
                "doc_id": item.doc_id,
                "score": item.score,
                "confidence": item.confidence,
                "node_ids": item.node_ids,
                "evidence_count": len(item.evidence),
            }
            if item.metrics:
                metadata["llm_calls"] = item.metrics.llm_calls
                metadata["rounds_used"] = item.metrics.rounds_used
                metadata["nodes_visited"] = item.metrics.nodes_visited
            documents.append(
                Document(
                    page_content=item.content,
                    metadata=metadata,
                )
            )
        return documents
