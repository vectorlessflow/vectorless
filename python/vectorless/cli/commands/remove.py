"""remove command — remove document index."""

import click


def remove_cmd(doc_id: str) -> None:
    """Remove a document from the index.

    Args:
        doc_id: Document identifier to remove.

    Uses:
        Engine.remove(doc_id)
    """
    raise NotImplementedError
