"""Protocol definitions for the ask pipeline.

Defines structural types via Protocol so that the Worker and Orchestrator
receive properly typed parameters instead of bare `Any`.
"""

from __future__ import annotations

from typing import Any, Protocol, runtime_checkable


# ---------------------------------------------------------------------------
# NavigableDocument — the interface Workers use to navigate documents
# ---------------------------------------------------------------------------

@runtime_checkable
class NavigableDocument(Protocol):
    """Structural type for documents that Workers can navigate.

    Implemented by the PyO3 ``Document`` class from vectorless-py.
    All methods are async and return native Python objects.
    """

    # Navigation
    async def ls(self) -> list[Any]:
        """List children of the current node."""
        ...

    async def cd(self, node_id: str) -> None:
        """Navigate into a child node by ID."""
        ...

    async def cd_by_title(self, title: str) -> None:
        """Navigate into a child node by title."""
        ...

    async def cd_up(self) -> None:
        """Navigate to the parent node."""
        ...

    async def back(self) -> None:
        """Navigate to the previously visited node."""
        ...

    # Content access
    async def cat(self, node_id: str | None = None) -> str:
        """Read the full content of a node."""
        ...

    async def head(self, node_id: str, n: int) -> str:
        """Read the first N lines of a node."""
        ...

    async def pwd(self) -> str:
        """Return the current navigation path as a string."""
        ...

    # Search
    async def find(self, keyword: str) -> list[Any]:
        """Find nodes whose titles match a keyword."""
        ...

    async def grep(self, pattern: str) -> list[Any]:
        """Search node contents for a pattern."""
        ...

    async def grep_node(self, node_id: str, pattern: str) -> list[Any]:
        """Search a specific node's content for a pattern."""
        ...

    async def keyword_entries(self, keyword: str) -> list[Any]:
        """Look up reasoning index entries for a keyword."""
        ...

    async def find_section(self, title: str) -> Any:
        """Find a section by exact or partial title match."""
        ...

    # Metadata
    async def toc(self, max_depth: int = 0) -> list[Any]:
        """Return table of contents entries."""
        ...

    async def stats(self, node_id: str | None = None) -> Any:
        """Return stats for a node."""
        ...

    async def similar(self, node_id: str) -> list[Any]:
        """Find similar nodes to the given node."""
        ...

    async def section_overview(self, node_id: str) -> str:
        """Get an overview of a section."""
        ...

    async def siblings(self, node_id: str) -> list[Any]:
        """Get sibling nodes of the given node."""
        ...

    async def ancestors(self, node_id: str) -> list[Any]:
        """Get ancestor nodes from root to the given node."""
        ...

    async def doc_card(self) -> Any:
        """Get the document card with metadata."""
        ...

    async def concepts(self) -> list[Any]:
        """Get extracted key concepts."""
        ...

    # Identity
    async def root_id(self) -> str:
        """Return the root node ID."""
        ...

    async def current_id(self) -> str:
        """Return the current node ID."""
        ...

    async def node_title(self, node_id: str) -> str:
        """Return the title of a node by ID."""
        ...

    async def doc_name(self) -> str:
        """Return the document name."""
        ...

    async def wc(self, node_id: str | None = None) -> Any:
        """Return word count info for a node."""
        ...

    # Agent acceleration (pre-computed from compile pipeline)
    async def intent_routes(self) -> list[Any]:
        """Get all intent routes from the query routing table."""
        ...

    async def concept_routes(self, keyword: str) -> list[Any]:
        """Get concept routes matching a keyword."""
        ...

    async def chains_for(self, node_id: str) -> list[Any]:
        """Get reasoning chains involving a specific node."""
        ...

    async def overlaps_for(self, node_id: str) -> list[Any]:
        """Get overlapping nodes for a specific node."""
        ...

    async def evidence_score(self, node_id: str) -> Any:
        """Get evidence quality score for a specific node."""
        ...

    async def evidence_scores_ranked(self) -> list[Any]:
        """Get all evidence scores ranked by composite score."""
        ...

    async def node_routing(self, node_id: str) -> Any:
        """Compile-time routing signal for a node: summary, keywords, question hints."""
        ...

    async def search(self, query: str, limit: int = 10) -> list[Any]:
        """Ranked full-text (BM25) search across the document; returns top hits."""
        ...


# ---------------------------------------------------------------------------
# Callable protocols
# ---------------------------------------------------------------------------

class DocLoader(Protocol):
    """Async callable that loads a navigable document by ID."""

    async def __call__(self, doc_id: str) -> NavigableDocument: ...


class EventCallback(Protocol):
    """Async callable that receives event dicts."""

    async def __call__(self, event: dict) -> Any: ...
