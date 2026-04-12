// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Python bindings for vectorless.

use pyo3::prelude::*;
use pyo3::exceptions::PyException;
use pyo3_async_runtimes::tokio::future_into_py;
use std::sync::Arc;
use tokio::runtime::Runtime;

use ::vectorless::client::{
    DocumentFormat, DocumentInfo, Engine, EngineBuilder, FailedItem, IndexContext, IndexItem,
    IndexMode, IndexOptions, IndexResult, QueryContext, QueryResult, QueryResultItem,
};
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

impl From<VectorlessError> for PyErr {
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

/// Parse format string to DocumentFormat.
fn parse_format(format: &str) -> PyResult<DocumentFormat> {
    match format.to_lowercase().as_str() {
        "markdown" | "md" => Ok(DocumentFormat::Markdown),
        "pdf" => Ok(DocumentFormat::Pdf),
        _ => Err(PyErr::from(VectorlessError::new(
            format!("Unknown format: {}. Supported: markdown, pdf", format),
            "config",
        ))),
    }
}

// ============================================================
// IndexOptions
// ============================================================

/// Options for controlling indexing behavior.
///
/// Args:
///     mode: Indexing mode - "default", "force", or "incremental".
///     generate_summaries: Whether to generate summaries. Default: True.
///     generate_description: Whether to generate document description. Default: False.
///     include_text: Whether to include node text in the tree. Default: True.
///     generate_ids: Whether to generate node IDs. Default: True.
#[pyclass(name = "IndexOptions", skip_from_py_object)]
#[derive(Clone)]
pub struct PyIndexOptions {
    inner: IndexOptions,
}

#[pymethods]
impl PyIndexOptions {
    #[new]
    #[pyo3(signature = (mode="default", generate_summaries=true, generate_description=false, include_text=true, generate_ids=true))]
    fn new(
        mode: &str,
        generate_summaries: bool,
        generate_description: bool,
        include_text: bool,
        generate_ids: bool,
    ) -> PyResult<Self> {
        let mut opts = IndexOptions::new();
        match mode {
            "default" => {}
            "force" => opts = opts.with_mode(IndexMode::Force),
            "incremental" => opts = opts.with_mode(IndexMode::Incremental),
            _ => {
                return Err(PyErr::from(VectorlessError::new(
                    format!("Unknown mode: {}. Supported: default, force, incremental", mode),
                    "config",
                )))
            }
        }
        opts.generate_summaries = generate_summaries;
        opts.generate_description = generate_description;
        opts.include_text = include_text;
        opts.generate_ids = generate_ids;
        Ok(Self { inner: opts })
    }

    fn __repr__(&self) -> String {
        format!(
            "IndexOptions(mode='{}', generate_summaries={}, generate_description={}, include_text={}, generate_ids={})",
            match self.inner.mode {
                IndexMode::Default => "default",
                IndexMode::Force => "force",
                IndexMode::Incremental => "incremental",
            },
            self.inner.generate_summaries,
            self.inner.generate_description,
            self.inner.include_text,
            self.inner.generate_ids,
        )
    }
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
/// # Single file
/// ctx = IndexContext.from_path("./document.pdf")
///
/// # Multiple files
/// ctx = IndexContext.from_paths(["./a.pdf", "./b.md"])
///
/// # Directory
/// ctx = IndexContext.from_dir("./docs/")
///
/// # From text
/// ctx = IndexContext.from_content("# Title\\nContent...", "markdown").with_name("doc")
///
/// # From bytes
/// ctx = IndexContext.from_bytes(data, "pdf").with_name("doc")
/// ```
#[pyclass(name = "IndexContext")]
pub struct PyIndexContext {
    inner: IndexContext,
}

#[pymethods]
impl PyIndexContext {
    /// Create an IndexContext from a single file path.
    #[staticmethod]
    fn from_path(path: String) -> Self {
        Self {
            inner: IndexContext::from_path(&path),
        }
    }

    /// Create an IndexContext from multiple file paths.
    #[staticmethod]
    fn from_paths(paths: Vec<String>) -> Self {
        Self {
            inner: IndexContext::from_paths(&paths),
        }
    }

    /// Create an IndexContext from all supported files in a directory.
    #[staticmethod]
    fn from_dir(path: String) -> Self {
        Self {
            inner: IndexContext::from_dir(&path),
        }
    }

    /// Create an IndexContext from text content.
    #[staticmethod]
    #[pyo3(signature = (content, format="markdown"))]
    fn from_content(content: String, format: &str) -> PyResult<Self> {
        let doc_format = parse_format(format)?;
        let ctx = IndexContext::from_content(&content, doc_format);
        Ok(Self { inner: ctx })
    }

    /// Create an IndexContext from binary data.
    #[staticmethod]
    fn from_bytes(data: Vec<u8>, format: &str) -> PyResult<Self> {
        let doc_format = parse_format(format)?;
        let ctx = IndexContext::from_bytes(data, doc_format);
        Ok(Self { inner: ctx })
    }

    /// Set the document name (single-source only).
    fn with_name(&self, name: String) -> Self {
        let ctx = self.inner.clone().with_name(&name);
        Self { inner: ctx }
    }

    /// Apply indexing options.
    fn with_options(&self, options: &PyIndexOptions) -> Self {
        let ctx = self.inner.clone().with_options(options.inner.clone());
        Self { inner: ctx }
    }

    /// Set indexing mode.
    fn with_mode(&self, mode: &str) -> PyResult<Self> {
        let m = match mode {
            "default" => IndexMode::Default,
            "force" => IndexMode::Force,
            "incremental" => IndexMode::Incremental,
            _ => {
                return Err(PyErr::from(VectorlessError::new(
                    format!("Unknown mode: {}. Supported: default, force, incremental", mode),
                    "config",
                )))
            }
        };
        let ctx = self.inner.clone().with_mode(m);
        Ok(Self { inner: ctx })
    }

    fn __repr__(&self) -> String {
        "IndexContext(...)".to_string()
    }
}

// ============================================================
// QueryContext
// ============================================================

/// Context for a query operation.
///
/// ```python
/// from vectorless import QueryContext
///
/// # Query a single document
/// ctx = QueryContext("What is the total revenue?").with_doc_id(doc_id)
///
/// # Query multiple documents
/// ctx = QueryContext("What is the architecture?").with_doc_ids(["doc-1", "doc-2"])
///
/// # Query entire workspace
/// ctx = QueryContext("Explain the algorithm")
/// ```
#[pyclass(name = "QueryContext")]
pub struct PyQueryContext {
    inner: QueryContext,
}

#[pymethods]
impl PyQueryContext {
    /// Create a new query context (defaults to workspace scope).
    #[new]
    fn new(query: String) -> Self {
        Self {
            inner: QueryContext::new(&query),
        }
    }

    /// Set scope to a single document.
    fn with_doc_id(&self, doc_id: String) -> Self {
        let ctx = self.inner.clone().with_doc_id(&doc_id);
        Self { inner: ctx }
    }

    /// Set scope to multiple documents.
    fn with_doc_ids(&self, doc_ids: Vec<String>) -> Self {
        let ctx = self.inner.clone().with_doc_ids(doc_ids);
        Self { inner: ctx }
    }

    /// Set scope to entire workspace.
    fn with_workspace(&self) -> Self {
        let ctx = self.inner.clone().with_workspace();
        Self { inner: ctx }
    }

    /// Set the maximum tokens for the result content.
    fn with_max_tokens(&self, tokens: usize) -> Self {
        let ctx = self.inner.clone().with_max_tokens(tokens);
        Self { inner: ctx }
    }

    /// Set whether to include the reasoning chain.
    fn with_include_reasoning(&self, include: bool) -> Self {
        let ctx = self.inner.clone().with_include_reasoning(include);
        Self { inner: ctx }
    }

    /// Set the maximum tree traversal depth.
    fn with_depth_limit(&self, depth: usize) -> Self {
        let ctx = self.inner.clone().with_depth_limit(depth);
        Self { inner: ctx }
    }

    fn __repr__(&self) -> String {
        "QueryContext(...)".to_string()
    }
}

// ============================================================
// QueryResultItem
// ============================================================

/// A single document's query result.
#[pyclass(name = "QueryResultItem")]
pub struct PyQueryResultItem {
    inner: QueryResultItem,
}

#[pymethods]
impl PyQueryResultItem {
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
            "QueryResultItem(doc_id='{}', score={:.2}, content_len={})",
            self.inner.doc_id,
            self.inner.score,
            self.inner.content.len()
        )
    }
}

// ============================================================
// FailedItem
// ============================================================

/// A failed item in a batch operation.
#[pyclass(name = "FailedItem")]
pub struct PyFailedItem {
    inner: FailedItem,
}

#[pymethods]
impl PyFailedItem {
    /// Source description.
    #[getter]
    fn source(&self) -> &str {
        &self.inner.source
    }

    /// Error message.
    #[getter]
    fn error(&self) -> &str {
        &self.inner.error
    }

    fn __repr__(&self) -> String {
        format!(
            "FailedItem(source='{}', error='{}')",
            self.inner.source, self.inner.error
        )
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
    /// Result items (one per document).
    #[getter]
    fn items(&self) -> Vec<PyQueryResultItem> {
        self.inner
            .items
            .iter()
            .map(|i| PyQueryResultItem {
                inner: i.clone(),
            })
            .collect()
    }

    /// Get the first (single-doc) result item.
    fn single(&self) -> Option<PyQueryResultItem> {
        self.inner.single().map(|i| PyQueryResultItem {
            inner: i.clone(),
        })
    }

    /// Number of result items.
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// Whether any documents failed.
    fn has_failures(&self) -> bool {
        self.inner.has_failures()
    }

    /// Failed items.
    #[getter]
    fn failed(&self) -> Vec<PyFailedItem> {
        self.inner
            .failed
            .iter()
            .map(|f| PyFailedItem { inner: f.clone() })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "QueryResult(items={}, failed={})",
            self.inner.len(),
            self.inner.failed.len()
        )
    }
}

// ============================================================
// IndexItem / IndexResult
// ============================================================

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

    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    #[getter]
    fn page_count(&self) -> Option<usize> {
        self.inner.page_count
    }

    fn __repr__(&self) -> String {
        format!(
            "IndexItem(doc_id='{}', name='{}')",
            self.inner.doc_id, self.inner.name
        )
    }
}

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

    /// Failed items.
    #[getter]
    fn failed(&self) -> Vec<PyFailedItem> {
        self.inner
            .failed
            .iter()
            .map(|f| PyFailedItem { inner: f.clone() })
            .collect()
    }

    /// Whether any items failed.
    fn has_failures(&self) -> bool {
        self.inner.has_failures()
    }

    /// Total number of items (successful + failed).
    fn total(&self) -> usize {
        self.inner.total()
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "IndexResult(doc_id={:?}, count={}, failed={})",
            self.inner.doc_id(),
            self.inner.items.len(),
            self.inner.failed.len()
        )
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
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn format(&self) -> &str {
        &self.inner.format
    }

    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    #[getter]
    fn page_count(&self) -> Option<usize> {
        self.inner.page_count
    }

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
// DocumentGraph types
// ============================================================

use ::vectorless::graph::{DocumentGraph, DocumentGraphNode, EdgeEvidence, GraphEdge, WeightedKeyword};

/// A keyword with weight from document analysis.
#[pyclass(name = "WeightedKeyword")]
pub struct PyWeightedKeyword {
    inner: WeightedKeyword,
}

#[pymethods]
impl PyWeightedKeyword {
    #[getter]
    fn keyword(&self) -> &str {
        &self.inner.keyword
    }

    #[getter]
    fn weight(&self) -> f32 {
        self.inner.weight
    }

    fn __repr__(&self) -> String {
        format!("WeightedKeyword('{}', weight={:.2})", self.inner.keyword, self.inner.weight)
    }
}

/// Evidence for a cross-document connection.
#[pyclass(name = "EdgeEvidence")]
pub struct PyEdgeEvidence {
    inner: EdgeEvidence,
}

#[pymethods]
impl PyEdgeEvidence {
    /// Number of shared keywords.
    #[getter]
    fn shared_keyword_count(&self) -> usize {
        self.inner.shared_keyword_count
    }

    /// Jaccard similarity of keyword sets.
    #[getter]
    fn keyword_jaccard(&self) -> f32 {
        self.inner.keyword_jaccard
    }

    /// Shared keywords with weights.
    #[getter]
    fn shared_keywords(&self) -> Vec<(String, f32, f32)> {
        self.inner
            .shared_keywords
            .iter()
            .map(|sk| (sk.keyword.clone(), sk.source_weight, sk.target_weight))
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "EdgeEvidence(shared={}, jaccard={:.2})",
            self.inner.shared_keyword_count, self.inner.keyword_jaccard
        )
    }
}

/// An edge representing a relationship between two documents.
#[pyclass(name = "GraphEdge")]
pub struct PyGraphEdge {
    inner: GraphEdge,
}

#[pymethods]
impl PyGraphEdge {
    /// Target document ID.
    #[getter]
    fn target_doc_id(&self) -> &str {
        &self.inner.target_doc_id
    }

    /// Edge weight (connection strength).
    #[getter]
    fn weight(&self) -> f32 {
        self.inner.weight
    }

    /// Evidence for this connection.
    #[getter]
    fn evidence(&self) -> PyEdgeEvidence {
        PyEdgeEvidence {
            inner: self.inner.evidence.clone(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "GraphEdge(target='{}', weight={:.2})",
            self.inner.target_doc_id, self.inner.weight
        )
    }
}

/// A node in the document graph representing an indexed document.
#[pyclass(name = "DocumentGraphNode")]
pub struct PyDocumentGraphNode {
    inner: DocumentGraphNode,
}

#[pymethods]
impl PyDocumentGraphNode {
    #[getter]
    fn doc_id(&self) -> &str {
        &self.inner.doc_id
    }

    #[getter]
    fn title(&self) -> &str {
        &self.inner.title
    }

    #[getter]
    fn format(&self) -> &str {
        &self.inner.format
    }

    #[getter]
    fn node_count(&self) -> usize {
        self.inner.node_count
    }

    /// Top keywords extracted from the document.
    #[getter]
    fn top_keywords(&self) -> Vec<PyWeightedKeyword> {
        self.inner
            .top_keywords
            .iter()
            .map(|kw| PyWeightedKeyword {
                inner: kw.clone(),
            })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "DocumentGraphNode(doc_id='{}', title='{}')",
            self.inner.doc_id, self.inner.title
        )
    }
}

/// Cross-document relationship graph.
///
/// Automatically rebuilt after indexing. Connects documents
/// that share keywords via Jaccard similarity.
#[pyclass(name = "DocumentGraph")]
pub struct PyDocumentGraph {
    inner: DocumentGraph,
}

#[pymethods]
impl PyDocumentGraph {
    /// Number of document nodes.
    fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    /// Number of relationship edges.
    fn edge_count(&self) -> usize {
        self.inner.edge_count()
    }

    /// Get a document node by ID.
    fn get_node(&self, doc_id: String) -> Option<PyDocumentGraphNode> {
        self.inner.get_node(&doc_id).map(|n| PyDocumentGraphNode {
            inner: n.clone(),
        })
    }

    /// Get all document IDs in the graph.
    fn doc_ids(&self) -> Vec<String> {
        self.inner.doc_ids().map(|s| s.to_string()).collect()
    }

    /// Get edges (neighbors) for a document.
    fn get_neighbors(&self, doc_id: String) -> Vec<PyGraphEdge> {
        self.inner
            .get_neighbors(&doc_id)
            .iter()
            .map(|e| PyGraphEdge {
                inner: e.clone(),
            })
            .collect()
    }

    /// Whether the graph is empty.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn __repr__(&self) -> String {
        format!(
            "DocumentGraph(nodes={}, edges={})",
            self.inner.node_count(),
            self.inner.edge_count()
        )
    }
}

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
///     workspace="./data",
///     api_key="sk-...",
///     model="gpt-4o",
/// )
///
/// # Index
/// result = await engine.index(IndexContext.from_path("./report.pdf"))
/// doc_id = result.doc_id
///
/// # Query
/// answer = await engine.query(QueryContext("What is the revenue?").with_doc_id(doc_id))
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
    ///     workspace: Path to the workspace directory.
    ///     config_path: Path to configuration file (optional).
    ///     api_key: **Required**. LLM API key.
    ///     model: **Required**. LLM model name.
    ///     endpoint: Optional API endpoint.
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

        let engine = rt.block_on(async {
            let mut builder = EngineBuilder::new();

            if let Some(path) = &config_path {
                builder = builder.with_config_path(path);
            }
            if let Some(ws) = &workspace {
                builder = builder.with_workspace(ws);
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
    fn query<'py>(
        &self,
        py: Python<'py>,
        ctx: &PyQueryContext,
    ) -> PyResult<Bound<'py, PyAny>> {
        let engine = Arc::clone(&self.inner);
        let query_ctx = ctx.inner.clone();
        future_into_py(py, run_query(engine, query_ctx))
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

    fn __repr__(&self) -> String {
        "Engine(...)".to_string()
    }
}

// ============================================================
// Module Definition
// ============================================================

/// Vectorless - Reasoning-native document intelligence engine.
///
/// ```python
/// from vectorless import Engine, IndexContext, QueryContext
///
/// engine = Engine(workspace="./data", api_key="sk-...", model="gpt-4o")
/// result = await engine.index(IndexContext.from_path("./report.pdf"))
/// answer = await engine.query(QueryContext("What is the revenue?").with_doc_id(result.doc_id))
/// print(answer.single().content)
/// ```
#[pymodule]
fn _vectorless(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VectorlessError>()?;
    m.add_class::<PyIndexOptions>()?;
    m.add_class::<PyIndexContext>()?;
    m.add_class::<PyQueryContext>()?;
    m.add_class::<PyIndexResult>()?;
    m.add_class::<PyIndexItem>()?;
    m.add_class::<PyQueryResult>()?;
    m.add_class::<PyQueryResultItem>()?;
    m.add_class::<PyFailedItem>()?;
    m.add_class::<PyDocumentInfo>()?;
    m.add_class::<PyDocumentGraphNode>()?;
    m.add_class::<PyDocumentGraph>()?;
    m.add_class::<PyGraphEdge>()?;
    m.add_class::<PyEdgeEvidence>()?;
    m.add_class::<PyWeightedKeyword>()?;
    m.add_class::<PyEngine>()?;

    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
