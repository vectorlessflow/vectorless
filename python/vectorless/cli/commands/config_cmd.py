"""config command — view and modify configuration."""

from typing import Optional

import click


def config_cmd(
    key: Optional[str] = None,
    value: Optional[str] = None,
    *,
    init_config: bool = False,
) -> None:
    """View or modify workspace configuration.

    Args:
        key: Config key (dot-separated, e.g. "llm.model").
        value: New value to set. If None, prints current value.
        init_config: Reset config to defaults.

    Usage:
        vectorless-cli config                    # show all
        vectorless-cli config llm.model          # show one key
        vectorless-cli config llm.model gpt-4o   # set value
        vectorless-cli config --init             # reset defaults

    Config keys (in .vectorless/config.toml):
        llm.model           LLM model name
        llm.api_key         API key (or env VECTORLESS_API_KEY)
        llm.endpoint        API endpoint
        retrieval.strategy  agent | pipeline
        retrieval.max_rounds  navigation budget
        index.summary       full | selective | lazy | navigation
        index.compact_mode  true | false
    """
    raise NotImplementedError


def _load_config(workspace: str) -> dict:
    """Load config.toml from workspace.

    Args:
        workspace: Path to .vectorless/ directory.

    Returns:
        Parsed config dict.
    """
    raise NotImplementedError


def _save_config(workspace: str, config: dict) -> None:
    """Save config dict to config.toml.

    Args:
        workspace: Path to .vectorless/ directory.
        config: Config dict to serialize.
    """
    raise NotImplementedError


def _default_config() -> dict:
    """Return default configuration values."""
    raise NotImplementedError
