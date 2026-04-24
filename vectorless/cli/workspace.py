"""Workspace management — .vectorless/ directory operations."""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any, Dict, Optional

import click

WORKSPACE_DIR = ".vectorless"
CONFIG_FILE = "config.toml"
DATA_DIR = "data"
CACHE_DIR = "cache"

_DEFAULT_CONFIG = """\
# Vectorless workspace configuration
# See https://vectorless.dev/docs/configuration

[llm]
# model = "gpt-4o"
# api_key = "sk-..."
# endpoint = "https://api.openai.com/v1"

[llm.throttle]
# max_concurrent_requests = 10
# requests_per_minute = 500

[retrieval]
# top_k = 3
# max_iterations = 10

[storage]
# workspace_dir = "~/.vectorless"

[metrics]
# enabled = true
"""


def find_workspace(start: str = ".") -> Optional[str]:
    """Find .vectorless/ directory by walking up from start.

    Args:
        start: Directory to start searching from.

    Returns:
        Absolute path to .vectorless/ if found, else None.
    """
    current = Path(start).resolve()
    while True:
        candidate = current / WORKSPACE_DIR
        if candidate.is_dir():
            return str(candidate)
        parent = current.parent
        if parent == current:
            return None
        current = parent


def init_workspace(target: str = ".") -> str:
    """Create .vectorless/ directory structure with default config.

    Args:
        target: Parent directory to create workspace in.

    Returns:
        Path to created .vectorless/ directory.

    Creates:
        target/.vectorless/
        ├── config.toml
        ├── data/
        └── cache/
    """
    workspace = Path(target).resolve() / WORKSPACE_DIR
    workspace.mkdir(parents=True, exist_ok=True)
    (workspace / DATA_DIR).mkdir(exist_ok=True)
    (workspace / CACHE_DIR).mkdir(exist_ok=True)

    config_path = workspace / CONFIG_FILE
    if not config_path.exists():
        config_path.write_text(_DEFAULT_CONFIG)

    return str(workspace)


def get_workspace_path(start: str = ".") -> str:
    """Get workspace path or raise.

    Args:
        start: Directory to search from.

    Returns:
        Absolute path to .vectorless/ directory.

    Raises:
        click.ClickException: If workspace not found.
    """
    path = find_workspace(start)
    if path is None:
        raise click.ClickException(
            "No .vectorless/ workspace found. Run 'vectorless init' first."
        )
    return path


def load_config(workspace: str) -> Dict[str, Any]:
    """Load configuration from workspace config.toml.

    Args:
        workspace: Path to .vectorless/ directory.

    Returns:
        Configuration dict.
    """
    config_path = Path(workspace) / CONFIG_FILE
    if not config_path.exists():
        return {}

    import tomllib

    with open(config_path, "rb") as f:
        return tomllib.load(f)


def save_config(workspace: str, config: Dict[str, Any]) -> None:
    """Save configuration to workspace config.toml.

    Args:
        workspace: Path to .vectorless/ directory.
        config: Configuration dict to save.
    """
    config_path = Path(workspace) / CONFIG_FILE
    lines: list[str] = []

    def _write_section(key: str, value: Any, prefix: str = "") -> None:
        section = f"{prefix}{key}" if not prefix else f"{prefix}.{key}"
        if isinstance(value, dict):
            lines.append(f"\n[{section}]")
            for k, v in value.items():
                _write_section(k, v, section)
        elif isinstance(value, str):
            lines.append(f'{key} = "{value}"')
        elif isinstance(value, bool):
            lines.append(f"{key} = {'true' if value else 'false'}")
        elif isinstance(value, (int, float)):
            lines.append(f"{key} = {value}")

    for k, v in config.items():
        _write_section(k, v)

    config_path.write_text("\n".join(lines) + "\n")


def get_data_dir(workspace: str) -> str:
    """Get data directory path within workspace."""
    return str(Path(workspace) / DATA_DIR)


def get_cache_dir(workspace: str) -> str:
    """Get cache directory path within workspace."""
    return str(Path(workspace) / CACHE_DIR)
