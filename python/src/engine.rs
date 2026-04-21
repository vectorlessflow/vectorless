// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Engine Python wrapper and async helpers.

use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::future_into_py;
use std::sync::Arc;
use tokio::runtime::Runtime;

use ::vectorless::{Engine, EngineBuilder, IndexContext, QueryContext};

use super::config::PyConfig;
use super::context::{PyIndexContext, PyQueryContext};
use super::document::PyDocumentInfo;
use super::error::VectorlessError;
use super::error::to_py_err;
use super::graph::PyDocumentGraph;
use super::metrics::PyMetricsReport;
use super::results::{PyIndexResult, PyQueryResult};
use super::streaming::PyStreamingQuery;

// ============================================================
// Engine async helpers (named functions to avoid FnOnce HRTB issue)
// ============================================================

async fn run_index(engine: Arc<Engine>, ctx: IndexContext) -> PyResult<PyIndexResult> {
    let result = engine.index(ctx).await.map_err(to_py_err)?;
    Ok(PyIndexResult { inner: result })
}

async fn run_query(engine: Arc<Engine>, ctx: QueryContext) -> PyResult<PyQueryResult> {
    let result = engine.query(ctx).await.map_err(to_py_err)?;
    Ok(PyQueryResult { inner: result })
}

async fn run_list(engine: Arc<Engine>) -> PyResult<Vec<PyDocumentInfo>> {
    let docs = engine.list().await.map_err(to_py_err)?;
    Ok(docs
        .into_iter()
        .map(|d| PyDocumentInfo { inner: d })
        .collect())
}

async fn run_remove(engine: Arc<Engine>, doc_id: String) -> PyResult<bool> {
    engine.remove(&doc_id).await.map_err(to_py_err)
}

async fn run_clear(engine: Arc<Engine>) -> PyResult<usize> {
    engine.clear().await.map_err(to_py_err)
}

async fn run_exists(engine: Arc<Engine>, doc_id: String) -> PyResult<bool> {
    engine.exists(&doc_id).await.map_err(to_py_err)
}

async fn run_get_graph(engine: Arc<Engine>) -> PyResult<Option<PyDocumentGraph>> {
    let graph = engine.get_graph().await.map_err(to_py_err)?;
    Ok(graph.map(|g| PyDocumentGraph { inner: g }))
}

async fn run_query_stream(engine: Arc<Engine>, ctx: QueryContext) -> PyResult<PyStreamingQuery> {
    let rx = engine.query_stream(ctx).await.map_err(to_py_err)?;
    Ok(PyStreamingQuery::new(rx))
}

fn run_metrics_report(engine: Arc<Engine>) -> PyMetricsReport {
    PyMetricsReport {
        inner: engine.metrics_report(),
    }
}

// ============================================================
// Engine
// ============================================================

/// The main vectorless engine.
///
/// `api_key` and `model` are **required**.
///
/// ```python
/// from vectorless import Engine, IndexContext, QueryContext
///
/// engine = Engine(
///     api_key="sk-...",
///     model="gpt-4o",
/// )
///
/// # Index
/// result = await engine.index(IndexContext.from_path("./report.pdf"))
/// doc_id = result.doc_id
///
/// # Query
/// answer = await engine.query(QueryContext("What is the revenue?").with_doc_ids([doc_id]))
/// print(answer.single().content)
/// ```
#[pyclass(name = "Engine")]
pub struct PyEngine {
    inner: Arc<Engine>,
}

#[pymethods]
impl PyEngine {
    /// Create a new Engine.
    ///
    /// Args:
    ///     api_key: **Required**. LLM API key.
    ///     model: **Required**. LLM model name.
    ///     endpoint: Optional API endpoint.
    ///     config: Optional Config for advanced tuning.
    ///
    /// Raises:
    ///     VectorlessError: If engine creation fails.
    #[new]
    #[pyo3(signature = (api_key=None, model=None, endpoint=None, config=None))]
    fn new(
        api_key: Option<String>,
        model: Option<String>,
        endpoint: Option<String>,
        config: Option<PyRef<PyConfig>>,
    ) -> PyResult<Self> {
        let rt = Runtime::new().map_err(|e| {
            PyErr::from(VectorlessError::new(
                format!("Failed to create tokio runtime: {}", e),
                "config",
            ))
        })?;

        let rust_config = config.map(|c| c.inner.clone());

        let engine = rt.block_on(async {
            let mut builder = EngineBuilder::new();

            if let Some(config) = rust_config {
                builder = builder.with_config(config);
            }

            if let Some(m) = &model {
                builder = builder.with_model(m);
            }
            if let Some(e) = &endpoint {
                builder = builder.with_endpoint(e);
            }
            if let Some(key) = api_key {
                builder = builder.with_key(key);
            }

            builder.build().await
        });

        let engine = engine.map_err(|e| {
            PyErr::from(VectorlessError::new(
                format!("Failed to create engine: {}", e),
                "config",
            ))
        })?;

        Ok(Self {
            inner: Arc::new(engine),
        })
    }

    /// Index a document.
    ///
    /// Args:
    ///     ctx: IndexContext created from from_path, from_paths, from_dir, etc.
    ///
    /// Returns:
    ///     IndexResult with doc_id and items.
    ///
    /// Raises:
    ///     VectorlessError: If indexing fails.
    fn index<'py>(&self, py: Python<'py>, ctx: &PyIndexContext) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        let index_ctx = ctx.inner.clone();
        future_into_py(py, run_index(engine, index_ctx))
    }

    /// Query indexed documents.
    ///
    /// Args:
    ///     ctx: QueryContext with query text and scope.
    ///
    /// Returns:
    ///     QueryResult with answer and score.
    ///
    /// Raises:
    ///     VectorlessError: If query fails.
    fn query<'py>(&self, py: Python<'py>, ctx: &PyQueryContext) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        let query_ctx = ctx.inner.clone();
        future_into_py(py, run_query(engine, query_ctx))
    }

    /// Query documents with streaming progress events.
    ///
    /// Returns a StreamingQuery async iterator that yields real-time
    /// retrieval events as dicts with a ``"type"`` key.
    ///
    /// Args:
    ///     ctx: QueryContext with query text and scope.
    ///
    /// Returns:
    ///     StreamingQuery async iterator.
    ///
    /// Raises:
    ///     VectorlessError: If query setup fails.
    fn query_stream<'py>(
        &self,
        py: Python<'py>,
        ctx: &PyQueryContext,
    ) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        let query_ctx = ctx.inner.clone();
        future_into_py(py, run_query_stream(engine, query_ctx))
    }

    /// List all indexed documents.
    ///
    /// Returns:
    ///     List of DocumentInfo objects.
    fn list<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        future_into_py(py, run_list(engine))
    }

    /// Remove a document by ID.
    ///
    /// Returns:
    ///     True if removed, False if not found.
    fn remove<'py>(&self, py: Python<'py>, doc_id: String) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        future_into_py(py, run_remove(engine, doc_id))
    }

    /// Remove all indexed documents.
    ///
    /// Returns:
    ///     Number of documents removed.
    fn clear<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        future_into_py(py, run_clear(engine))
    }

    /// Check if a document exists.
    fn exists<'py>(&self, py: Python<'py>, doc_id: String) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        future_into_py(py, run_exists(engine, doc_id))
    }

    /// Get the cross-document relationship graph.
    ///
    /// Returns:
    ///     DocumentGraph if any documents exist, else None.
    fn get_graph<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        future_into_py(py, run_get_graph(engine))
    }

    /// Generate a complete metrics report.
    fn metrics_report(&self) -> PyMetricsReport {
        run_metrics_report(Arc::clone(&self.inner))
    }

    fn __repr__(&self) -> String {
        "Engine(...)".to_string()
    }
}
