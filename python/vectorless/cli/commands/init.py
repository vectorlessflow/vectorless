"""init command — initialize .vectorless/ workspace."""

import click

from vectorless.cli.workspace import init_workspace


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
    try:
        path = init_workspace(workspace)
        click.echo(f"Initialized Vectorless workspace at {path}")
        click.echo("Edit config.toml to set your LLM API key and model.")
    except Exception as e:
        raise click.ClickException(f"Failed to initialize workspace: {e}") from e
