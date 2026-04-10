// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Python bindings for vectorless.
//!
//! This module provides Python bindings using PyO3.

use pyo3::prelude::*;
use pyo3::exceptions::PyException;
use std::sync::Arc;
use tokio::runtime::Runtime;

// Use ::vectorless to avoid conflict with the #[pymodule] named vectorless
use ::vectorless::client::{Engine, EngineBuilder, IndexContext, IndexItem, IndexResult, QueryContext, QueryResult, DocumentInfo};
use ::vectorless::client::DocumentFormat;
use ::vectorless::error::Error as RustError;

// ============================================================
// Error Types
// ============================================================

/// Python exception for vectorless errors.
#[pyclass(extends = PyException, subclass)]
pub struct VectorlessError {
    message: String,
    kind: String,
}

#[pymethods]
impl VectorlessError {
    #[new]
    fn new_py(message: String, kind: String) -> Self {
        Self { message, kind }
    }

    #[getter]
    fn message(&self) -> &str {
        &self.message
    }

    #[getter]
    fn kind(&self) -> &str {
        &self.kind
    }

    fn __str__(&self) -> &str {
        &self.message
    }

    fn __repr__(&self) -> String {
        format!("VectorlessError('{}', kind='{}')", self.message, self.kind)
    }
}

impl VectorlessError {
    fn new(message: String, kind: &str) -> Self {
        Self {
            message,
            kind: kind.to_string(),
        }
    }
}

impl std::convert::From<VectorlessError> for PyErr {
    fn from(err: VectorlessError) -> PyErr {
        PyErr::new::<VectorlessError, _>((err.message, err.kind))
    }
}

/// Convert vectorless errors to Python exceptions.
fn to_py_err(e: RustError) -> PyErr {
    let message = e.to_string();
    let kind = match &e {
        RustError::DocumentNotFound(_) => "not_found",
        RustError::Parse(_) => "parse",
        RustError::Config(_) => "config",
        RustError::Workspace(_) => "workspace",
        RustError::Llm(_) => "llm",
        _ => "unknown",
    };
    VectorlessError::new(message, kind).into()
}

// ============================================================
// IndexContext
// ============================================================

/// Context for indexing a document.
///
/// Create using the static methods:
///
/// ```python
/// from vectorless import IndexContext
///
/// # From file
/// ctx = IndexContext.from_file("./document.pdf")
///
/// # From text
/// ctx = IndexContext.from_text("# Title\\nContent...", name="doc")
///
/// # From bytes
/// ctx = IndexContext.from_bytes(data, name="doc", format="pdf")
/// ```
#[pyclass(name = "IndexContext")]
pub struct PyIndexContext {
    inner: IndexContext,
}

#[pymethods]
impl PyIndexContext {
    /// Create an IndexContext from a file path.
    ///
    /// The format is detected from the file extension.
    ///
    /// Args:
    ///     path: Path to the file.
    ///     name: Optional document name.
    ///
    /// Returns:
    ///     IndexContext for the file.
    #[staticmethod]
    #[pyo3(signature = (path, name=None))]
    fn from_file(path: String, name: Option<String>) -> Self {
        let mut ctx = IndexContext::from_path(&path);
        if let Some(n) = name {
            ctx = ctx.with_name(&n);
        }
        Self { inner: ctx }
    }

    /// Create an IndexContext from text content.
    ///
    /// Args:
    ///     content: The text content.
    ///     name: Optional document name.
    ///     format: Content format ("markdown", "html", "text"). Default: "markdown".
    ///
    /// Returns:
    ///     IndexContext for the content.
    #[staticmethod]
    #[pyo3(signature = (content, name=None, format="markdown"))]
    fn from_content(content: String, name: Option<String>, format: &str) -> PyResult<Self> {
        let doc_format = parse_format(format)?;
        let mut ctx = IndexContext::from_content(&content, doc_format);
        if let Some(n) = name {
            ctx = ctx.with_name(&n);
        }
        Ok(Self { inner: ctx })
    }

    /// Create an IndexContext from binary data.
    ///
    /// Args:
    ///     data: The binary data.
    ///     name: Document name (required).
    ///     format: Content format ("pdf", "docx").
    ///
    /// Returns:
    ///     IndexContext for the bytes.
    #[staticmethod]
    #[pyo3(signature = (data, name, format))]
    fn from_bytes(data: Vec<u8>, name: String, format: &str) -> PyResult<Self> {
        let doc_format = parse_format(format)?;
        let ctx = IndexContext::from_bytes(data, doc_format).with_name(&name);
        Ok(Self { inner: ctx })
    }
}

/// Parse format string to DocumentFormat.
fn parse_format(format: &str) -> PyResult<DocumentFormat> {
    match format.to_lowercase().as_str() {
        "markdown" | "md" => Ok(DocumentFormat::Markdown),
        "pdf" => Ok(DocumentFormat::Pdf),
        "docx" | "doc" => Ok(DocumentFormat::Docx),
        "html" | "htm" => Ok(DocumentFormat::Html),
        _ => Err(PyErr::from(VectorlessError::new(
            format!("Unknown format: {}. Supported: markdown, pdf, docx, html", format),
            "config",
        ))),
    }
}

// ============================================================
// QueryResult
// ============================================================

/// Result of a document query.
#[pyclass(name = "QueryResult")]
pub struct PyQueryResult {
    inner: QueryResult,
}

#[pymethods]
impl PyQueryResult {
    /// The document ID.
    #[getter]
    fn doc_id(&self) -> &str {
        &self.inner.doc_id
    }

    /// The retrieved content.
    #[getter]
    fn content(&self) -> &str {
        &self.inner.content
    }

    /// Relevance score (0.0 to 1.0).
    #[getter]
    fn score(&self) -> f32 {
        self.inner.score
    }

    /// Node IDs that matched.
    #[getter]
    fn node_ids(&self) -> Vec<String> {
        self.inner.node_ids.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "QueryResult(doc_id='{}', score={:.2}, content_len={})",
            self.inner.doc_id,
            self.inner.score,
            self.inner.content.len()
        )
    }
}

// ============================================================
// IndexResult
// ============================================================

/// Result of a document indexing operation.
#[pyclass(name = "IndexResult")]
pub struct PyIndexResult {
    inner: IndexResult,
}

#[pymethods]
impl PyIndexResult {
    /// The document ID (convenience for single-document indexing).
    #[getter]
    fn doc_id(&self) -> Option<String> {
        self.inner.doc_id().map(|s| s.to_string())
    }

    /// All indexed items.
    #[getter]
    fn items(&self) -> Vec<PyIndexItem> {
        self.inner
            .items
            .iter()
            .map(|i| PyIndexItem { inner: i.clone() })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "IndexResult(doc_id={:?}, count={})",
            self.inner.doc_id(),
            self.inner.items.len()
        )
    }
}

/// A single indexed document item.
#[pyclass(name = "IndexItem")]
pub struct PyIndexItem {
    inner: IndexItem,
}

#[pymethods]
impl PyIndexItem {
    #[getter]
    fn doc_id(&self) -> &str {
        &self.inner.doc_id
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn format(&self) -> String {
        format!("{:?}", self.inner.format).to_lowercase()
    }

    fn __repr__(&self) -> String {
        format!("IndexItem(doc_id='{}', name='{}')", self.inner.doc_id, self.inner.name)
    }
}

// ============================================================
// DocumentInfo
// ============================================================

/// Information about an indexed document.
#[pyclass(name = "DocumentInfo")]
pub struct PyDocumentInfo {
    inner: DocumentInfo,
}

#[pymethods]
impl PyDocumentInfo {
    /// Document ID.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Document name.
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// Document format.
    #[getter]
    fn format(&self) -> &str {
        &self.inner.format
    }

    /// Document description (if available).
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    /// Page count (for PDFs).
    #[getter]
    fn page_count(&self) -> Option<usize> {
        self.inner.page_count
    }

    /// Line count (for text files).
    #[getter]
    fn line_count(&self) -> Option<usize> {
        self.inner.line_count
    }

    fn __repr__(&self) -> String {
        format!(
            "DocumentInfo(id='{}', name='{}', format='{}')",
            self.inner.id, self.inner.name, self.inner.format
        )
    }
}

// ============================================================
// Engine
// ============================================================

/// The main vectorless engine.
///
/// Configuration priority (later overrides earlier):
/// 1. Default configuration
/// 2. Explicit config file (config_path parameter)
/// 3. Environment variables (OPENAI_API_KEY, VECTORLESS_MODEL, etc.)
/// 4. Constructor parameters (api_key, model, endpoint) - highest priority
///
/// # Zero Configuration (Recommended)
///
/// Just set OPENAI_API_KEY environment variable:
///
/// ```python
/// from vectorless import Engine
///
/// engine = Engine(workspace="./data")
/// ```
///
/// # With Custom Model
///
/// ```python
/// engine = Engine(workspace="./data", model="gpt-4o-mini")
/// ```
///
/// # With Config File (Advanced)
///
/// ```python
/// engine = Engine(workspace="./data", config_path="./vectorless.toml")
/// ```
#[pyclass(name = "Engine")]
pub struct PyEngine {
    inner: Arc<Engine>,
    rt: Runtime,
}

#[pymethods]
impl PyEngine {
    /// Create a new Engine.
    ///
    /// Args:
    ///     workspace: Path to the workspace directory (optional if config_path provides it).
    ///     config_path: Path to configuration file (optional, advanced usage).
    ///     api_key: Optional API key. If not provided, uses OPENAI_API_KEY env var.
    ///     model: Optional model name. Default: "gpt-4o".
    ///     endpoint: Optional API endpoint.
    ///
    /// Configuration priority (later overrides earlier):
    ///     1. Default configuration
    ///     2. config_path parameter (if provided)
    ///     3. Environment variables (OPENAI_API_KEY, VECTORLESS_MODEL, etc.)
    ///     4. Constructor parameters (api_key, model, endpoint)
    ///
    /// Raises:
    ///     VectorlessError: If engine creation fails.
    #[new]
    #[pyo3(signature = (workspace=None, config_path=None, api_key=None, model=None, endpoint=None))]
    fn new(
        workspace: Option<String>,
        config_path: Option<String>,
        api_key: Option<String>,
        model: Option<String>,
        endpoint: Option<String>,
    ) -> PyResult<Self> {
        let rt = Runtime::new().map_err(|e| {
            PyErr::from(VectorlessError::new(
                format!("Failed to create tokio runtime: {}", e),
                "config",
            ))
        })?;

        // Resolve API key: explicit > env var
        let resolved_api_key = api_key.or_else(|| std::env::var("OPENAI_API_KEY").ok());

        let engine = rt.block_on(async {
            let mut builder = EngineBuilder::new();

            // Set config path first (if provided)
            if let Some(path) = &config_path {
                builder = builder.with_config_path(path);
            }

            // Set workspace (if provided)
            if let Some(ws) = &workspace {
                builder = builder.with_workspace(ws);
            }

            if let Some(m) = &model {
                builder = builder.with_model(m);
            }

            if let Some(e) = &endpoint {
                builder = builder.with_endpoint(e);
            }

            if let Some(key) = resolved_api_key {
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
            rt,
        })
    }

    /// Index a document.
    ///
    /// Args:
    ///     ctx: IndexContext created from from_file, from_text, or from_bytes.
    ///
    /// Returns:
    ///     IndexResult with doc_id and metadata.
    ///
    /// Raises:
    ///     VectorlessError: If indexing fails.
    fn index(&self, ctx: &PyIndexContext) -> PyResult<PyIndexResult> {
        let engine = Arc::clone(&self.inner);
        let index_ctx = ctx.inner.clone();

        let result = self.rt.block_on(async move {
            engine.index(index_ctx).await.map_err(to_py_err)
        })?;

        Ok(PyIndexResult { inner: result })
    }

    /// Query a document.
    ///
    /// Args:
    ///     doc_id: Document ID returned from index().
    ///     question: The question to ask.
    ///
    /// Returns:
    ///     QueryResult with the answer.
    ///
    /// Raises:
    ///     VectorlessError: If query fails.
    fn query(&self, doc_id: String, question: String) -> PyResult<PyQueryResult> {
        let engine = Arc::clone(&self.inner);

        let ctx = QueryContext::new(&question).with_doc_id(&doc_id);

        let result = self.rt.block_on(async move {
            engine.query(ctx).await.map_err(to_py_err)
        })?;

        Ok(PyQueryResult { inner: result })
    }

    /// List all indexed documents.
    ///
    /// Returns:
    ///     List of DocumentInfo objects.
    ///
    /// Raises:
    ///     VectorlessError: If listing fails.
    fn list(&self) -> PyResult<Vec<PyDocumentInfo>> {
        let engine = Arc::clone(&self.inner);

        let docs = self.rt.block_on(async move {
            engine.list().await.map_err(to_py_err)
        })?;

        Ok(docs
            .into_iter()
            .map(|d| PyDocumentInfo { inner: d })
            .collect())
    }

    /// Remove a document.
    ///
    /// Args:
    ///     doc_id: Document ID to remove.
    ///
    /// Returns:
    ///     True if document was removed, False if not found.
    ///
    /// Raises:
    ///     VectorlessError: If removal fails.
    fn remove(&self, doc_id: String) -> PyResult<bool> {
        let engine = Arc::clone(&self.inner);

        self.rt.block_on(async move {
            engine.remove(&doc_id).await.map_err(to_py_err)
        })
    }

    /// Clear all documents.
    ///
    /// Returns:
    ///     Number of documents removed.
    ///
    /// Raises:
    ///     VectorlessError: If clearing fails.
    fn clear(&self) -> PyResult<usize> {
        let engine = Arc::clone(&self.inner);

        self.rt.block_on(async move { engine.clear().await.map_err(to_py_err) })
    }

    /// Check if a document exists.
    ///
    /// Args:
    ///     doc_id: Document ID to check.
    ///
    /// Returns:
    ///     True if document exists.
    fn exists(&self, doc_id: String) -> PyResult<bool> {
        let engine = Arc::clone(&self.inner);

        self.rt.block_on(async move { engine.exists(&doc_id).await.map_err(to_py_err) })
    }

    fn __repr__(&self) -> String {
        "Engine(workspace=...)".to_string()
    }
}

// ============================================================
// Module Definition
// ============================================================

/// Vectorless - Reasoning-native document intelligence engine.
///
/// Quick Start:
///
/// ```python
/// from vectorless import Engine, IndexContext
///
/// # Create engine
/// engine = Engine(workspace="./data")
///
/// # Index a document
/// ctx = IndexContext.from_file("./report.pdf")
/// result = engine.index(ctx)
///
/// # Query
/// answer = engine.query(result.doc_id, "What is the revenue?")
/// print(answer.content)
/// ```
#[pymodule]
fn vectorless(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VectorlessError>()?;
    m.add_class::<PyIndexContext>()?;
    m.add_class::<PyIndexResult>()?;
    m.add_class::<PyIndexItem>()?;
    m.add_class::<PyQueryResult>()?;
    m.add_class::<PyDocumentInfo>()?;
    m.add_class::<PyEngine>()?;

    // Add version
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
