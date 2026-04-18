"""stats command — workspace statistics."""

import click


def stats_cmd() -> None:
    """Show workspace statistics.

    Displays:
        - Workspace path
        - Number of indexed documents
        - Total nodes / leaves / tokens
        - Index size on disk
        - DocumentGraph info (edges, connected components)
        - Last indexed timestamp

    Uses:
        Engine.list() -> count documents
        Engine.metrics_report()
        Filesystem scan for size info
    """
    raise NotImplementedError
