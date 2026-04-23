// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Engine Python wrapper — async ingest/ask/forget/list_documents.

use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::future_into_py;
use std::sync::Arc;
use tokio::runtime::Runtime;

use ::vectorless::{Engine, EngineBuilder, IngestInput};

use super::answer::PyAnswer;
use super::document::PyDocumentInfo;
use super::error::VectorlessError;
use super::error::to_py_err;
use super::graph::PyDocumentGraph;
use super::metrics::PyMetricsReport;

// ============================================================
// Engine async helpers (named functions to avoid FnOnce HRTB issue)
// ============================================================

async fn run_ingest(engine: Arc<Engine>, input: IngestInput) -> PyResult<PyDocumentInfo> {
    let doc = engine.ingest(input).await.map_err(to_py_err)?;
    Ok(PyDocumentInfo { inner: doc })
}

async fn run_ask(
    engine: Arc<Engine>,
    question: String,
    doc_ids: Vec<String>,
) -> PyResult<PyAnswer> {
    let answer = engine.ask(&question, &doc_ids).await.map_err(to_py_err)?;
    Ok(PyAnswer { inner: answer })
}

async fn run_forget(engine: Arc<Engine>, doc_id: String) -> PyResult<()> {
    engine.forget(&doc_id).await.map_err(to_py_err)
}

async fn run_list_documents(engine: Arc<Engine>) -> PyResult<Vec<PyDocumentInfo>> {
    let docs = engine.list_documents().await.map_err(to_py_err)?;
    Ok(docs
        .into_iter()
        .map(|d| PyDocumentInfo { inner: d })
        .collect())
}

async fn run_exists(engine: Arc<Engine>, doc_id: String) -> PyResult<bool> {
    engine.exists(&doc_id).await.map_err(to_py_err)
}

async fn run_clear(engine: Arc<Engine>) -> PyResult<usize> {
    engine.clear().await.map_err(to_py_err)
}

async fn run_get_graph(engine: Arc<Engine>) -> PyResult<Option<PyDocumentGraph>> {
    let graph = engine.get_graph().await.map_err(to_py_err)?;
    Ok(graph.map(|g| PyDocumentGraph { inner: g }))
}

fn run_metrics_report(engine: Arc<Engine>) -> PyMetricsReport {
    PyMetricsReport {
        inner: engine.metrics_report(),
    }
}

// ============================================================
// Engine
// ============================================================

/// The vectorless Document Understanding Engine.
///
/// All methods are **async** — use `await` to call them.
///
/// ```python
/// from vectorless import Engine
///
/// engine = Engine(api_key="sk-...", model="gpt-4o")
///
/// # Understand a document
/// doc = await engine.ingest("./report.pdf")
/// print(doc.summary)
///
/// # Ask a question
/// answer = await engine.ask("What is the revenue?", doc_ids=[doc.doc_id])
/// print(answer.content)
/// print(answer.trace)  # reasoning trace — always present
///
/// # List all understood documents
/// docs = await engine.list_documents()
///
/// # Forget a document
/// await engine.forget(doc.doc_id)
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
        config: Option<PyRef<super::config::PyConfig>>,
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

    /// Understand a document — parse, analyze, and persist.
    ///
    /// Args:
    ///     path: File path to the document (PDF or Markdown).
    ///
    /// Returns:
    ///     DocumentInfo with doc_id, summary, structure, concepts.
    ///
    /// Raises:
    ///     VectorlessError: If ingest fails.
    fn ingest<'py>(&self, py: Python<'py>, path: String) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        let input = IngestInput::Path(path.into());
        future_into_py(py, run_ingest(engine, input))
    }

    /// Ask a question — returns a reasoned answer with evidence and trace.
    ///
    /// Args:
    ///     question: The question to ask (required).
    ///     doc_ids: List of document IDs to search. Empty = search all.
    ///
    /// Returns:
    ///     Answer with content, evidence, confidence, and trace.
    ///
    /// Raises:
    ///     VectorlessError: If ask fails.
    #[pyo3(signature = (question, doc_ids=None))]
    fn ask<'py>(
        &self,
        py: Python<'py>,
        question: String,
        doc_ids: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        let ids = doc_ids.unwrap_or_default();
        future_into_py(py, run_ask(engine, question, ids))
    }

    /// Remove a document by ID.
    ///
    /// Args:
    ///     doc_id: The document ID to remove.
    ///
    /// Raises:
    ///     VectorlessError: If removal fails.
    fn forget<'py>(&self, py: Python<'py>, doc_id: String) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        future_into_py(py, run_forget(engine, doc_id))
    }

    /// List all understood documents.
    ///
    /// Returns:
    ///     List of DocumentInfo objects with summary, structure, and concepts.
    fn list_documents<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        future_into_py(py, run_list_documents(engine))
    }

    /// Check if a document exists.
    fn exists<'py>(&self, py: Python<'py>, doc_id: String) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        future_into_py(py, run_exists(engine, doc_id))
    }

    /// Remove all documents.
    ///
    /// Returns:
    ///     Number of documents removed.
    fn clear<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        future_into_py(py, run_clear(engine))
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
