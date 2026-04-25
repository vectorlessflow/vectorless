// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Config Python wrapper.

use pyo3::prelude::*;

/// Advanced configuration for Engine internals.
///
/// Create a Config to customize storage, concurrency,
/// and other engine parameters beyond the basic builder API.
///
/// Example:
///
/// ```python
/// from vectorless import Config, Engine
///
/// config = Config()
/// config.set_workspace_dir("/data/vectorless")
/// config.set_max_concurrent_requests(20)
///
/// engine = Engine(api_key="sk-...", model="gpt-4o", config=config)
/// ```
#[pyclass(name = "Config", skip_from_py_object)]
pub struct PyConfig {
    pub(crate) inner: vectorless_engine::Config,
}

#[pymethods]
impl PyConfig {
    /// Create a new Config with defaults.
    #[new]
    fn new() -> Self {
        Self {
            inner: vectorless_engine::Config::default(),
        }
    }

    /// Set the workspace directory for persisted documents.
    ///
    /// Default: ~/.vectorless
    fn set_workspace_dir(&mut self, dir: &str) {
        self.inner.storage.workspace_dir = std::path::PathBuf::from(dir);
    }

    /// Set the maximum concurrent LLM API calls.
    ///
    /// Default: 10
    fn set_max_concurrent_requests(&mut self, max: usize) {
        self.inner.llm.throttle.max_concurrent_requests = max;
    }

    /// Set the rate limit (requests per minute).
    ///
    /// Default: 500
    fn set_requests_per_minute(&mut self, rpm: usize) {
        self.inner.llm.throttle.requests_per_minute = rpm;
    }

    /// Enable or disable metrics collection.
    ///
    /// Default: True
    fn set_metrics_enabled(&mut self, enabled: bool) {
        self.inner.metrics.enabled = enabled;
    }
}
