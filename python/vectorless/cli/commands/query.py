"""query command — single query (maps to engine.query)."""

from typing import Optional

import click


def query_cmd(
    question: str,
    *,
    doc_ids: tuple[str, ...] = (),
    workspace_scope: bool = False,
    fmt: str = "text",
    verbose: bool = False,
    timeout_secs: Optional[int] = None,
) -> None:
    """Execute a single query against indexed documents.

    Args:
        question: Natural-language question.
        doc_ids: Limit to specific document IDs.
        workspace_scope: Query across all documents.
        fmt: Output format — "text" or "json".
        verbose: Show Agent navigation steps.
        timeout_secs: Per-operation timeout in seconds.

    Uses:
        Engine.query(QueryContext(question)
            .with_doc_ids([...])  or  .with_workspace()
            .with_timeout_secs(n))
        -> QueryResult
    """
    raise NotImplementedError
