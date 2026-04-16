// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! IndexContext, QueryContext, and IndexOptions Python wrappers.

use pyo3::prelude::*;

use ::vectorless::client::{DocumentFormat, IndexContext, IndexMode, IndexOptions, QueryContext};

use super::error::VectorlessError;

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
///     enable_synonym_expansion: Whether to expand keywords with LLM-generated
///         synonyms during indexing. Improves recall for differently-worded queries.
///         Default: False.
#[pyclass(name = "IndexOptions", skip_from_py_object)]
#[derive(Clone)]
pub struct PyIndexOptions {
    pub(crate) inner: IndexOptions,
}

#[pymethods]
impl PyIndexOptions {
    #[new]
    #[pyo3(signature = (mode="default", generate_summaries=true, generate_description=false, include_text=true, generate_ids=true, enable_synonym_expansion=false))]
    fn new(
        mode: &str,
        generate_summaries: bool,
        generate_description: bool,
        include_text: bool,
        generate_ids: bool,
        enable_synonym_expansion: bool,
    ) -> PyResult<Self> {
        let mut opts = IndexOptions::new();
        match mode {
            "default" => {}
            "force" => opts = opts.with_mode(IndexMode::Force),
            "incremental" => opts = opts.with_mode(IndexMode::Incremental),
            _ => {
                return Err(PyErr::from(VectorlessError::new(
                    format!(
                        "Unknown mode: {}. Supported: default, force, incremental",
                        mode
                    ),
                    "config",
                )));
            }
        }
        opts.generate_summaries = generate_summaries;
        opts.generate_description = generate_description;
        opts.include_text = include_text;
        opts.generate_ids = generate_ids;
        opts.enable_synonym_expansion = enable_synonym_expansion;
        Ok(Self { inner: opts })
    }

    fn __repr__(&self) -> String {
        format!(
            "IndexOptions(mode='{}', generate_summaries={}, generate_description={}, include_text={}, generate_ids={}, enable_synonym_expansion={})",
            match self.inner.mode {
                IndexMode::Default => "default",
                IndexMode::Force => "force",
                IndexMode::Incremental => "incremental",
            },
            self.inner.generate_summaries,
            self.inner.generate_description,
            self.inner.include_text,
            self.inner.generate_ids,
            self.inner.enable_synonym_expansion,
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
    pub(crate) inner: IndexContext,
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
    ///
    /// Args:
    ///     path: Directory path to scan.
    ///     recursive: If True, scan subdirectories recursively. Default: False.
    #[staticmethod]
    #[pyo3(signature = (path, recursive=false))]
    fn from_dir(path: String, recursive: bool) -> Self {
        let inner = IndexContext::from_dir(&path, recursive);
        Self { inner }
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
                    format!(
                        "Unknown mode: {}. Supported: default, force, incremental",
                        mode
                    ),
                    "config",
                )));
            }
        };
        let ctx = self.inner.clone().with_mode(m);
        Ok(Self { inner: ctx })
    }

    /// Number of document sources.
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// Whether no sources are present.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn __repr__(&self) -> String {
        format!("IndexContext(sources={})", self.inner.len())
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
/// # Query specific documents
/// ctx = QueryContext("What is the total revenue?").with_doc_ids([doc_id])
///
/// # Query multiple documents
/// ctx = QueryContext("What is the architecture?").with_doc_ids(["doc-1", "doc-2"])
///
/// # Query entire workspace
/// ctx = QueryContext("Explain the algorithm")
/// ```
#[pyclass(name = "QueryContext")]
pub struct PyQueryContext {
    pub(crate) inner: QueryContext,
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

    /// Set scope to specific documents.
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
