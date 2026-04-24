"""Pydantic configuration models for Vectorless Engine."""

from __future__ import annotations

from typing import Optional

from pydantic import BaseModel, Field

from vectorless._internal._core import Config as RustConfig


class ThrottleConfig(BaseModel):
    """LLM request throttling."""

    max_concurrent_requests: int = 10
    requests_per_minute: int = 500


class RetryConfig(BaseModel):
    """LLM request retry policy."""

    max_attempts: int = 3
    initial_delay_secs: float = 1.0
    max_delay_secs: float = 30.0


class LlmConfig(BaseModel):
    """LLM connection configuration."""

    model: str = ""
    api_key: Optional[str] = None
    endpoint: Optional[str] = None
    throttle: ThrottleConfig = ThrottleConfig()
    retry: RetryConfig = RetryConfig()


class MetricsConfig(BaseModel):
    """Metrics collection configuration."""

    enabled: bool = True


class StorageConfig(BaseModel):
    """Storage and workspace configuration."""

    workspace_dir: str = "~/.vectorless"


class EngineConfig(BaseModel):
    """Full engine configuration.

    Usage::

        from vectorless import EngineConfig

        config = EngineConfig(
            llm=LlmConfig(model="gpt-4o", api_key="sk-..."),
        )

        # Convert to Rust Config for Engine construction
        rust_config = config.to_rust_config()
    """

    llm: LlmConfig = LlmConfig()
    metrics: MetricsConfig = MetricsConfig()
    storage: StorageConfig = StorageConfig()

    def to_rust_config(self) -> RustConfig:
        """Convert to the Rust-backed Config object.

        Calls the setter methods defined in python/src/config.rs.
        """
        cfg = RustConfig()
        cfg.set_workspace_dir(self.storage.workspace_dir)
        cfg.set_max_concurrent_requests(self.llm.throttle.max_concurrent_requests)
        cfg.set_requests_per_minute(self.llm.throttle.requests_per_minute)
        cfg.set_metrics_enabled(self.metrics.enabled)
        return cfg
