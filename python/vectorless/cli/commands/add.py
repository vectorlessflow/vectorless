"""add command — index documents (maps to engine.index)."""

from typing import Optional

import click


def add_cmd(
    path: str,
    *,
    recursive: bool = False,
    fmt: Optional[str] = None,
    force: bool = False,
    jobs: int = 1,
    verbose: bool = False,
) -> None:
    """Index a document or directory.

    Args:
        path: File or directory path.
        recursive: Index directory recursively.
        fmt: Force format ("markdown" | "pdf" | None for auto-detect).
        force: Force re-index existing documents.
        jobs: Number of parallel indexing jobs.
        verbose: Show detailed progress.

    Uses:
        Engine.index(IndexContext)
        IndexContext.from_path / from_paths / from_dir
        IndexOptions(mode="force" if force else "default")
    """
    raise NotImplementedError
