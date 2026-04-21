"""config command — view and modify configuration."""

import sys
from pathlib import Path
from typing import Any, Dict, Optional

import click

from vectorless.cli.workspace import get_workspace_path, load_config, save_config


def _default_config() -> dict:
    """Return default configuration values."""
    return {
        "llm": {
            "model": "",
            "api_key": "",
            "endpoint": "",
            "throttle": {
                "max_concurrent_requests": 10,
                "requests_per_minute": 500,
            },
        },
        "retrieval": {
            "top_k": 3,
            "max_iterations": 10,
        },
        "storage": {
            "workspace_dir": "~/.vectorless",
        },
        "metrics": {
            "enabled": True,
        },
    }


def _deep_get(cfg: dict, dotted_key: str) -> Any:
    """Get a nested value from a dict using dot-separated key.

    Args:
        cfg: Configuration dict.
        dotted_key: Dot-separated key, e.g. "llm.model".

    Returns:
        The value at the key path, or None if not found.
    """
    parts = dotted_key.split(".")
    current = cfg
    for part in parts:
        if not isinstance(current, dict) or part not in current:
            return None
        current = current[part]
    return current


def _deep_set(cfg: dict, dotted_key: str, value: Any) -> None:
    """Set a nested value in a dict using dot-separated key.

    Args:
        cfg: Configuration dict.
        dotted_key: Dot-separated key, e.g. "llm.model".
        value: Value to set.
    """
    parts = dotted_key.split(".")
    current = cfg
    for part in parts[:-1]:
        if part not in current or not isinstance(current[part], dict):
            current[part] = {}
        current = current[part]
    current[parts[-1]] = value


def _coerce_value(value: str) -> Any:
    """Attempt to coerce a string value to its proper type.

    Args:
        value: String value from CLI input.

    Returns:
        Coerced value (bool, int, float, or str).
    """
    # Boolean
    if value.lower() in ("true", "yes", "1"):
        return True
    if value.lower() in ("false", "no", "0"):
        return False
    # Integer
    try:
        return int(value)
    except ValueError:
        pass
    # Float
    try:
        return float(value)
    except ValueError:
        pass
    return value


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
        retrieval.strategy  agent
        retrieval.max_rounds  navigation budget
        index.summary       full | selective | lazy | navigation
        index.compact_mode  true | false
    """
    workspace = get_workspace_path()

    if init_config:
        defaults = _default_config()
        save_config(workspace, defaults)
        click.echo("Configuration reset to defaults.")
        return

    config = load_config(workspace)

    if key is None:
        # Show all config
        if not config:
            click.echo("Configuration is empty. Use --init to set defaults.")
            return

        # Pretty-print the config
        _print_config(config)
        return

    if value is None:
        # Show a specific key
        val = _deep_get(config, key)
        if val is None:
            raise click.ClickException(f"Config key not found: {key}")
        if isinstance(val, dict):
            _print_config(val, indent="  ")
        else:
            click.echo(f"{key} = {val}")
        return

    # Set a key to a value
    coerced = _coerce_value(value)
    _deep_set(config, key, coerced)
    save_config(workspace, config)
    click.echo(f"Set {key} = {coerced}")


def _print_config(cfg: dict, indent: str = "") -> None:
    """Recursively print config dict.

    Args:
        cfg: Config dict to print.
        indent: Current indentation prefix.
    """
    for k, v in cfg.items():
        if isinstance(v, dict):
            click.echo(f"{indent}{k}:")
            _print_config(v, indent=indent + "  ")
        else:
            # Mask API keys
            if "api_key" in k.lower() and isinstance(v, str) and v:
                display = v[:8] + "..." if len(v) > 8 else "***"
            else:
                display = v
            click.echo(f"{indent}{k} = {display}")
