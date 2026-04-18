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
    max_tokens: Optional[int] = None,
) -> None:
    """Execute a single query against indexed documents.

    Args:
        question: Natural-language question.
        doc_ids: Limit to specific document IDs.
        workspace_scope: Query across all documents.
        fmt: Output format — "text" or "json".
        verbose: Show Agent navigation steps.
        max_tokens: Max result tokens.

    Uses:
        Engine.query(QueryContext(question)
            .with_doc_ids([...])  or  .with_workspace()
            .with_max_tokens(n))
        -> QueryResult

    Verbose mode prints Agent navigation:
        [1/8] Bird's-eye: 3 top-level branches
        [2/8] Descend → payment-configuration
        [3/8] GetContent → doc 29139b
        [4/8] Evaluate → sufficient
        → Answer: ...
    """
    raise NotImplementedError
