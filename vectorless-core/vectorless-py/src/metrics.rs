// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Metrics report Python wrappers.

use pyo3::prelude::*;

use ::vectorless_engine::{LlmMetricsReport, MetricsReport, RetrievalMetricsReport};

/// LLM usage metrics report.
#[pyclass(name = "LlmMetricsReport", skip_from_py_object)]
pub struct PyLlmMetricsReport {
    pub(crate) inner: LlmMetricsReport,
}

#[pymethods]
impl PyLlmMetricsReport {
    /// Total number of LLM calls.
    #[getter]
    fn total_calls(&self) -> u64 {
        self.inner.total_calls
    }

    /// Number of successful calls.
    #[getter]
    fn successful_calls(&self) -> u64 {
        self.inner.successful_calls
    }

    /// Number of failed calls.
    #[getter]
    fn failed_calls(&self) -> u64 {
        self.inner.failed_calls
    }

    /// Success rate (0.0 - 1.0).
    #[getter]
    fn success_rate(&self) -> f64 {
        self.inner.success_rate
    }

    /// Total input tokens.
    #[getter]
    fn total_input_tokens(&self) -> u64 {
        self.inner.total_input_tokens
    }

    /// Total output tokens.
    #[getter]
    fn total_output_tokens(&self) -> u64 {
        self.inner.total_output_tokens
    }

    /// Total tokens (input + output).
    #[getter]
    fn total_tokens(&self) -> u64 {
        self.inner.total_tokens
    }

    /// Average latency per call in milliseconds.
    #[getter]
    fn avg_latency_ms(&self) -> f64 {
        self.inner.avg_latency_ms
    }

    /// Total latency in milliseconds.
    #[getter]
    fn total_latency_ms(&self) -> u64 {
        self.inner.total_latency_ms
    }

    /// Estimated cost in USD.
    #[getter]
    fn estimated_cost_usd(&self) -> f64 {
        self.inner.estimated_cost_usd
    }

    /// Number of rate limit errors.
    #[getter]
    fn rate_limit_errors(&self) -> u64 {
        self.inner.rate_limit_errors
    }

    /// Number of timeout errors.
    #[getter]
    fn timeout_errors(&self) -> u64 {
        self.inner.timeout_errors
    }

    /// Number of fallback triggers.
    #[getter]
    fn fallback_triggers(&self) -> u64 {
        self.inner.fallback_triggers
    }

    fn __repr__(&self) -> String {
        format!(
            "LlmMetricsReport(calls={}, tokens={}, cost=${:.4})",
            self.inner.total_calls, self.inner.total_tokens, self.inner.estimated_cost_usd,
        )
    }
}

/// Retrieval operation metrics report.
#[pyclass(name = "RetrievalMetricsReport", skip_from_py_object)]
pub struct PyRetrievalMetricsReport {
    pub(crate) inner: RetrievalMetricsReport,
}

#[pymethods]
impl PyRetrievalMetricsReport {
    /// Total number of queries.
    #[getter]
    fn total_queries(&self) -> u64 {
        self.inner.total_queries
    }

    /// Total number of search iterations.
    #[getter]
    fn total_iterations(&self) -> u64 {
        self.inner.total_iterations
    }

    /// Average iterations per query.
    #[getter]
    fn avg_iterations(&self) -> f64 {
        self.inner.avg_iterations
    }

    /// Total nodes visited.
    #[getter]
    fn nodes_visited(&self) -> u64 {
        self.inner.nodes_visited
    }

    /// Total paths found.
    #[getter]
    fn paths_found(&self) -> u64 {
        self.inner.paths_found
    }

    /// Average path length.
    #[getter]
    fn avg_path_length(&self) -> f64 {
        self.inner.avg_path_length
    }

    /// Average path score (0.0 - 1.0).
    #[getter]
    fn avg_path_score(&self) -> f64 {
        self.inner.avg_path_score
    }

    /// Number of high-score paths (>= 0.5).
    #[getter]
    fn high_score_paths(&self) -> u64 {
        self.inner.high_score_paths
    }

    /// Number of low-score paths (< 0.3).
    #[getter]
    fn low_score_paths(&self) -> u64 {
        self.inner.low_score_paths
    }

    /// Number of cache hits.
    #[getter]
    fn cache_hits(&self) -> u64 {
        self.inner.cache_hits
    }

    /// Number of cache misses.
    #[getter]
    fn cache_misses(&self) -> u64 {
        self.inner.cache_misses
    }

    /// Cache hit rate (0.0 - 1.0).
    #[getter]
    fn cache_hit_rate(&self) -> f64 {
        self.inner.cache_hit_rate
    }

    /// Total latency in milliseconds.
    #[getter]
    fn total_latency_ms(&self) -> u64 {
        self.inner.total_latency_ms
    }

    /// Average latency per query in milliseconds.
    #[getter]
    fn avg_latency_ms(&self) -> f64 {
        self.inner.avg_latency_ms
    }

    /// Number of backtracks.
    #[getter]
    fn backtracks(&self) -> u64 {
        self.inner.backtracks
    }

    /// Number of sufficiency checks.
    #[getter]
    fn sufficiency_checks(&self) -> u64 {
        self.inner.sufficiency_checks
    }

    /// Sufficiency rate (0.0 - 1.0).
    #[getter]
    fn sufficiency_rate(&self) -> f64 {
        self.inner.sufficiency_rate
    }

    fn __repr__(&self) -> String {
        format!(
            "RetrievalMetricsReport(queries={}, avg_score={:.2}, cache_hit={:.1}%)",
            self.inner.total_queries,
            self.inner.avg_path_score,
            self.inner.cache_hit_rate * 100.0,
        )
    }
}

/// Complete metrics report combining all subsystem metrics.
#[pyclass(name = "MetricsReport", skip_from_py_object)]
pub struct PyMetricsReport {
    pub(crate) inner: MetricsReport,
}

#[pymethods]
impl PyMetricsReport {
    /// LLM metrics.
    #[getter]
    fn llm(&self) -> PyLlmMetricsReport {
        PyLlmMetricsReport {
            inner: self.inner.llm.clone(),
        }
    }

    /// Retrieval metrics.
    #[getter]
    fn retrieval(&self) -> PyRetrievalMetricsReport {
        PyRetrievalMetricsReport {
            inner: self.inner.retrieval.clone(),
        }
    }

    /// Total estimated cost in USD.
    fn total_cost_usd(&self) -> f64 {
        self.inner.total_cost_usd()
    }

    fn __repr__(&self) -> String {
        format!(
            "MetricsReport(llm_calls={}, cost=${:.4}, queries={})",
            self.inner.llm.total_calls,
            self.inner.total_cost_usd(),
            self.inner.retrieval.total_queries,
        )
    }
}
