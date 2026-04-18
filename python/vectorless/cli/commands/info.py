"""info command — show document index details."""

import click


def info_cmd(doc_id: str) -> None:
    """Show detailed information about an indexed document.

    Args:
        doc_id: Document identifier.

    Uses:
        Engine.list() -> filter by doc_id
        Display: title, source, format, node count, depth, leaf count,
                 total tokens, routing keywords, top-level sections,
                 indexed timestamp.

    Example output:
        Document: API Guide (a1b2c3)
        Source: ./docs/api-guide.md
        Format: Markdown
        Tree: 45 nodes, depth 4, 12 leaves
        Total tokens: 8,234
        Routing keywords: api, authentication, endpoints, rate-limit
        Top-level sections:
          1. Overview (12 leaves)
          2. Authentication (8 leaves)
          3. Endpoints (18 leaves)
    """
    raise NotImplementedError
