"""tree command — visualize document tree structure."""

from typing import Optional

import click


def tree_cmd(
    doc_id: str,
    *,
    depth: Optional[int] = None,
    show_summary: bool = False,
    show_keywords: bool = False,
) -> None:
    """Visualize the hierarchical tree structure of an indexed document.

    Args:
        doc_id: Document identifier.
        depth: Max depth to display (None = full tree).
        show_summary: Include node summaries in output.
        show_keywords: Include routing keywords in output.

    Example output:
        API Guide (a1b2c3) — 45 nodes, 12 leaves
        1. Overview [routing: api-overview] (12 leaves)
        ├── 1.1 Introduction
        ├── 1.2 Authentication [keywords: auth, token, api-key]
        │   ├── 1.2.1 API Key Setup
        │   └── 1.2.2 OAuth Flow
        └── 1.3 Endpoints (18 leaves)
    """
    raise NotImplementedError
