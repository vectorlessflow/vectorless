"""Configuration loading from environment variables and TOML files."""

from __future__ import annotations

import os
import tomllib
from pathlib import Path
from typing import Any

from vectorless.config.models import EngineConfig, LlmConfig, StorageConfig


def load_config_from_env(prefix: str = "VECTORLESS_") -> EngineConfig:
    """Load configuration from environment variables.

    Recognized variables::

        VECTORLESS_API_KEY         -> llm.api_key
        VECTORLESS_MODEL           -> llm.model
        VECTORLESS_ENDPOINT        -> llm.endpoint
        VECTORLESS_WORKSPACE_DIR   -> storage.workspace_dir
        VECTORLESS_METRICS_ENABLED -> metrics.enabled
    """
    llm = LlmConfig()
    storage = StorageConfig()
    metrics_enabled: bool | None = None

    env_map = {
        f"{prefix}API_KEY": ("llm.api_key", str),
        f"{prefix}MODEL": ("llm.model", str),
        f"{prefix}ENDPOINT": ("llm.endpoint", str),
        f"{prefix}WORKSPACE_DIR": ("storage.workspace_dir", str),
        f"{prefix}METRICS_ENABLED": ("metrics.enabled", bool),
    }

    kwargs: dict[str, Any] = {}

    for env_key, (path, type_fn) in env_map.items():
        value = os.environ.get(env_key)
        if value is not None:
            if type_fn is bool:
                kwargs[path] = value.lower() in ("1", "true", "yes")
            else:
                kwargs[path] = type_fn(value)

    # Apply to sub-models
    if "llm.api_key" in kwargs:
        llm = LlmConfig(
            api_key=kwargs["llm.api_key"],
            model=kwargs.get("llm.model", llm.model),
            endpoint=kwargs.get("llm.endpoint", llm.endpoint),
        )
    elif "llm.model" in kwargs or "llm.endpoint" in kwargs:
        llm = LlmConfig(
            model=kwargs.get("llm.model", llm.model),
            endpoint=kwargs.get("llm.endpoint", llm.endpoint),
        )

    if "storage.workspace_dir" in kwargs:
        storage = StorageConfig(workspace_dir=kwargs["storage.workspace_dir"])

    if "metrics.enabled" in kwargs:
        from vectorless.config.models import MetricsConfig

        metrics = MetricsConfig(enabled=kwargs["metrics.enabled"])
    else:
        from vectorless.config.models import MetricsConfig

        metrics = MetricsConfig()

    return EngineConfig(llm=llm, storage=storage, metrics=metrics)


def load_config_from_file(path: Path) -> EngineConfig:
    """Load configuration from a TOML file.

    Expected format::

        [llm]
        model = "gpt-4o"
        api_key = "sk-..."
        endpoint = "https://api.openai.com/v1"

        [llm.throttle]
        max_concurrent_requests = 10
        requests_per_minute = 500

        [storage]
        workspace_dir = "~/.vectorless"

        [metrics]
        enabled = true
    """
    if tomllib is None:
        raise ImportError(
            "TOML parsing requires Python >= 3.11 (tomllib built-in)."
        )

    with open(path, "rb") as f:
        data = tomllib.load(f)

    return EngineConfig(**data)


def load_config(
    config_file: Path | None = None,
    env_prefix: str = "VECTORLESS_",
    overrides: dict[str, Any] | None = None,
) -> EngineConfig:
    """Load configuration with layered precedence.

    Merge order (later overrides earlier):
        defaults -> config file -> environment variables -> overrides dict
    """
    # Start with defaults
    config_data: dict[str, Any] = {}

    # Layer 1: config file
    if config_file is not None and config_file.exists():
        if tomllib is None:
            raise ImportError(
                "TOML parsing requires Python >= 3.11 (tomllib built-in)."
            )
        with open(config_file, "rb") as f:
            file_data = tomllib.load(f)
        config_data.update(file_data)

    # Layer 2: environment variables
    env_config = load_config_from_env(prefix=env_prefix)
    base = EngineConfig()
    env_data: dict[str, Any] = {}
    if env_config.llm.api_key != base.llm.api_key or env_config.llm.model != base.llm.model:
        env_data["llm"] = env_config.llm.model_dump()
    if env_config.storage.workspace_dir != base.storage.workspace_dir:
        env_data["storage"] = env_config.storage.model_dump()

    config_data.update(env_data)

    # Layer 3: explicit overrides
    if overrides:
        config_data.update(overrides)

    return EngineConfig(**config_data)
