"""init command — initialize .vectorless/ workspace."""

import click


def init_cmd(workspace: str) -> None:
    """Create .vectorless/ directory structure with default config.

    Creates:
        .vectorless/
        ├── config.toml        # LLM key/model/endpoint, retrieval strategy
        ├── data/              # Index data (DocumentTree, ReasoningIndex)
        └── cache/             # Memo cache

    Args:
        workspace: Parent directory to create .vectorless/ in.
    """
    raise NotImplementedError
