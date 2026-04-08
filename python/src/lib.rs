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
use ::vectorless::client::{Engine, EngineBuilder, IndexContext, QueryResult, DocumentInfo};
use ::vectorless::parser::DocumentFormat;
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
#[pyclass]
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
    fn from_text(content: String, name: Option<String>, format: &str) -> PyResult<Self> {
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
        "text" | "txt" => Ok(DocumentFormat::Text),
        _ => Err(PyErr::from(VectorlessError::new(
            format!("Unknown format: {}", format),
            "config",
        ))),
    }
}

// ============================================================
// QueryResult
// ============================================================

/// Result of a document query.
#[pyclass]
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
// DocumentInfo
// ============================================================

/// Information about an indexed document.
#[pyclass]
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
/// 2. Auto-detected config file (vectorless.toml, config.toml, .vectorless.toml)
/// 3. Explicit config file (config_path parameter)
/// 4. Environment variables (OPENAI_API_KEY, VECTORLESS_MODEL, etc.)
/// 5. Constructor parameters (api_key, model, endpoint) - highest priority
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
/// # With Full Config File (Advanced)
///
/// ```python
/// engine = Engine(config_path="./vectorless.toml")
/// ```
#[pyclass]
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
    ///     2. Auto-detected config file
    ///     3. config_path parameter
    ///     4. Environment variables (OPENAI_API_KEY, VECTORLESS_MODEL, etc.)
    ///     5. Constructor parameters (api_key, model, endpoint)
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

            // Set API key
            if let Some(key) = resolved_api_key {
                builder = builder.with_openai(key);
            }

            // Set model
            if let Some(m) = &model {
                builder = builder.with_model(m, None);
            }

            // Set endpoint
            if let Some(e) = &endpoint {
                builder = builder.with_endpoint(e);
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
    ///     Document ID string.
    ///
    /// Raises:
    ///     VectorlessError: If indexing fails.
    fn index(&self, ctx: &PyIndexContext) -> PyResult<String> {
        let engine = Arc::clone(&self.inner);
        let index_ctx = ctx.inner.clone();

        self.rt.block_on(async move {
            engine.index(index_ctx).await.map_err(to_py_err)
        })
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

        let result = self.rt.block_on(async move {
            engine.query(&doc_id, &question).await.map_err(to_py_err)
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
    fn list_docs(&self) -> PyResult<Vec<PyDocumentInfo>> {
        let engine = Arc::clone(&self.inner);

        let docs = self.rt.block_on(async move {
            engine.list_documents().await.map_err(to_py_err)
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

    /// Get the number of indexed documents.
    fn len(&self) -> PyResult<usize> {
        let engine = Arc::clone(&self.inner);

        self.rt.block_on(async move { engine.len().await.map_err(to_py_err) })
    }

    fn __repr__(&self) -> String {
        "Engine(workspace=...)".to_string()
    }
}

// ============================================================
// Module Definition
// ============================================================

/// Vectorless - Hierarchical document intelligence without vectors.
///
/// A document intelligence engine that uses tree-based understanding
/// instead of vector databases.
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
/// doc_id = engine.index(ctx)
///
/// # Query
/// result = engine.query(doc_id, "What is the revenue?")
/// print(result.content)
/// ```
#[pymodule]
fn _vectorless(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VectorlessError>()?;
    m.add_class::<PyIndexContext>()?;
    m.add_class::<PyQueryResult>()?;
    m.add_class::<PyDocumentInfo>()?;
    m.add_class::<PyEngine>()?;

    // Add version
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
