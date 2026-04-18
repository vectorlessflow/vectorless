"""list command — list indexed documents."""

import click


def list_cmd(*, fmt: str = "table") -> None:
    """List all indexed documents in the workspace.

    Args:
        fmt: Output format — "table" or "json".

    Uses:
        Engine.list() -> List[DocumentInfo]

    Table output:
        Doc ID | Title | Format | Nodes | Pages | Indexed At
    """
    raise NotImplementedError
