"""Tests for configuration models and loading."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

import pytest

from vectorless.config.models import (
    EngineConfig,
    LlmConfig,
    MetricsConfig,
    RetrievalConfig,
    StorageConfig,
)


class TestEngineConfig:
    def test_defaults(self):
        config = EngineConfig()
        assert config.llm.model == ""
        assert config.llm.api_key is None
        assert config.retrieval.top_k == 3
        assert config.storage.workspace_dir == "~/.vectorless"
        assert config.metrics.enabled is True

    def test_custom_values(self):
        config = EngineConfig(
            llm=LlmConfig(model="gpt-4o", api_key="sk-test"),
            retrieval=RetrievalConfig(top_k=10),
            storage=StorageConfig(workspace_dir="/data/vl"),
        )
        assert config.llm.model == "gpt-4o"
        assert config.llm.api_key == "sk-test"
        assert config.retrieval.top_k == 10
        assert config.storage.workspace_dir == "/data/vl"

    def test_to_rust_config(self):
        config = EngineConfig(
            llm=LlmConfig(model="gpt-4o", api_key="sk-test"),
            retrieval=RetrievalConfig(top_k=5, max_iterations=20),
            storage=StorageConfig(workspace_dir="/tmp/vl"),
            metrics=MetricsConfig(enabled=False),
        )
        # to_rust_config should not raise
        rust_config = config.to_rust_config()
        assert rust_config is not None

    def test_validation_top_k_minimum(self):
        with pytest.raises(Exception):
            RetrievalConfig(top_k=0)

    def test_json_roundtrip(self):
        config = EngineConfig(
            llm=LlmConfig(model="gpt-4o", api_key="sk-test"),
        )
        data = config.model_dump()
        restored = EngineConfig(**data)
        assert restored.llm.model == "gpt-4o"
        assert restored.llm.api_key == "sk-test"


class TestConfigLoading:
    def test_load_from_env(self):
        os.environ["VECTORLESS_API_KEY"] = "sk-env-test"
        os.environ["VECTORLESS_MODEL"] = "gpt-4o-mini"
        os.environ["VECTORLESS_TOP_K"] = "7"

        try:
            from vectorless.config.loading import load_config_from_env

            config = load_config_from_env()
            assert config.llm.api_key == "sk-env-test"
            assert config.llm.model == "gpt-4o-mini"
            assert config.retrieval.top_k == 7
        finally:
            del os.environ["VECTORLESS_API_KEY"]
            del os.environ["VECTORLESS_MODEL"]
            del os.environ["VECTORLESS_TOP_K"]

    def test_load_from_file(self):
        with tempfile.NamedTemporaryFile(mode="wb", suffix=".toml", delete=False) as f:
            f.write(b'[llm]\nmodel = "gpt-4o"\napi_key = "sk-file"\n')
            f.flush()

            try:
                from vectorless.config.loading import load_config_from_file

                config = load_config_from_file(Path(f.name))
                assert config.llm.model == "gpt-4o"
                assert config.llm.api_key == "sk-file"
            finally:
                os.unlink(f.name)
