// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Public API types for the client module.
//!
//! This module contains all types exposed in the public API.

use serde::{Deserialize, Serialize};

use vectorless_document::DocumentFormat;
use vectorless_metrics::CompileMetrics;

// ============================================================
// Partial Success
// ============================================================

/// A failed item in a batch operation.
#[derive(Debug, Clone)]
pub struct FailedItem {
    /// Source description (file path, content name, or doc ID).
    pub source: String,
    /// Error message.
    pub error: String,
}

impl FailedItem {
    /// Create a new failed item.
    pub fn new(source: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            error: error.into(),
        }
    }
}

// ============================================================
// Compile Types
// ============================================================

/// Document compilation behavior mode.
///
/// Controls how the compiler handles existing documents and re-compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompileMode {
    /// Default mode - skip if already compiled.
    ///
    /// If a document with the same source has already been compiled,
    /// the operation is skipped and the existing document ID is returned.
    #[default]
    Default,

    /// Force re-compiling.
    ///
    /// Always re-compile the document, even if it has been compiled before.
    /// A new document ID is generated.
    Force,

    /// Incremental mode - only re-compile changed files.
    ///
    /// Re-compile only if the file has been modified since the last compilation.
    /// For content/bytes sources, this behaves like [`CompileMode::Default`].
    Incremental,
}

/// Options for compiling a document.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Compilation mode.
    pub mode: CompileMode,

    /// Whether to generate summaries using LLM.
    pub generate_summaries: bool,

    /// Whether to generate node IDs.
    pub generate_ids: bool,

    /// Whether to generate document description.
    pub generate_description: bool,

    /// Whether to expand keywords with LLM-generated synonyms
    /// during reasoning index construction. Improves recall for
    /// queries that use different wording than the document.
    pub enable_synonym_expansion: bool,

    /// Per-operation timeout (seconds). `None` means no timeout.
    pub timeout_secs: Option<u64>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            mode: CompileMode::Default,
            generate_summaries: true,
            generate_ids: true,
            generate_description: true,
            enable_synonym_expansion: true,
            timeout_secs: None,
        }
    }
}

impl CompileOptions {
    /// Create new compile options with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable summary generation.
    pub fn with_summaries(mut self) -> Self {
        self.generate_summaries = true;
        self
    }

    /// Enable document description generation.
    pub fn with_description(mut self) -> Self {
        self.generate_description = true;
        self
    }

    /// Set the compilation mode.
    ///
    /// # Modes
    ///
    /// - [`CompileMode::Default`] - Skip if already compiled
    /// - [`CompileMode::Force`] - Always re-compile
    /// - [`CompileMode::Incremental`] - Only re-compile changed files
    pub fn with_mode(mut self, mode: CompileMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set per-operation timeout in seconds.
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }
}

// ============================================================
// Compile Result Types
// ============================================================

/// Result of a document compilation operation.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    /// Successfully compiled items.
    pub items: Vec<CompileArtifact>,

    /// Items that failed to index (partial success).
    pub failed: Vec<FailedItem>,
}

impl CompileOutput {
    /// Create a new compile result.
    pub fn new(items: Vec<CompileArtifact>) -> Self {
        Self {
            items,
            failed: Vec::new(),
        }
    }

    /// Create with both successes and failures.
    pub fn with_partial(items: Vec<CompileArtifact>, failed: Vec<FailedItem>) -> Self {
        Self { items, failed }
    }

    /// Get the single document ID (convenience for single-document compilation).
    pub fn doc_id(&self) -> Option<&str> {
        if self.items.len() == 1 {
            Some(&self.items[0].doc_id)
        } else {
            None
        }
    }

    /// Check if the result is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the number of compiled items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether any items failed.
    pub fn has_failures(&self) -> bool {
        !self.failed.is_empty()
    }

    /// Total number of sources (success + failed).
    pub fn total(&self) -> usize {
        self.items.len() + self.failed.len()
    }
}

/// A single compiled document item.
#[derive(Debug, Clone)]
pub struct CompileArtifact {
    /// The unique document ID.
    pub doc_id: String,
    /// The document name.
    pub name: String,
    /// The document format.
    pub format: DocumentFormat,
    /// Document description (from root summary).
    pub description: Option<String>,
    /// Source file path (if compiled from a file).
    pub source_path: Option<String>,
    /// Page count (for PDFs).
    pub page_count: Option<usize>,
    /// Compilation pipeline metrics (timing, LLM usage, node stats).
    pub metrics: Option<CompileMetrics>,
}

impl CompileArtifact {
    /// Create a new compile artifact.
    pub fn new(
        doc_id: impl Into<String>,
        name: impl Into<String>,
        format: DocumentFormat,
        description: Option<String>,
        page_count: Option<usize>,
    ) -> Self {
        Self {
            doc_id: doc_id.into(),
            name: name.into(),
            format,
            description,
            source_path: None,
            page_count,
            metrics: None,
        }
    }

    /// Set the source file path.
    pub fn with_source_path(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    /// Set the compilation metrics.
    pub fn with_metrics(mut self, metrics: CompileMetrics) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Set the compilation metrics (optional).
    pub fn with_metrics_opt(mut self, metrics: Option<CompileMetrics>) -> Self {
        self.metrics = metrics;
        self
    }
}

// ============================================================
// Document Info Types
// ============================================================

/// Document info for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentInfo {
    /// Document ID.
    pub id: String,

    /// Document name.
    pub name: String,

    /// Document format.
    pub format: String,

    /// Document description.
    pub description: Option<String>,

    /// Source file path.
    pub source_path: Option<String>,

    /// Page count (for PDFs).
    pub page_count: Option<usize>,

    /// Line count (for text files).
    pub line_count: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_options() {
        let options = CompileOptions::new()
            .with_summaries()
            .with_mode(CompileMode::Force);

        assert!(options.generate_summaries);
        assert_eq!(options.mode, CompileMode::Force);
    }

    #[test]
    fn test_compile_options_timeout() {
        let opts = CompileOptions::new().with_timeout_secs(30);
        assert_eq!(opts.timeout_secs, Some(30));

        let default = CompileOptions::default();
        assert_eq!(default.timeout_secs, None);
    }

    #[test]
    fn test_compile_result() {
        let item = CompileArtifact::new("doc-1", "Test", DocumentFormat::Markdown, None, None);
        let result = CompileOutput::new(vec![item]);

        assert_eq!(result.doc_id(), Some("doc-1"));
        assert_eq!(result.len(), 1);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_compile_result_empty() {
        let result = CompileOutput::new(vec![]);
        assert!(result.is_empty());
        assert_eq!(result.doc_id(), None);
    }

    #[test]
    fn test_compile_result_multiple() {
        let items = vec![
            CompileArtifact::new("doc-1", "A", DocumentFormat::Markdown, None, None),
            CompileArtifact::new("doc-2", "B", DocumentFormat::Pdf, None, None),
        ];
        let result = CompileOutput::new(items);
        assert_eq!(result.len(), 2);
        assert_eq!(result.doc_id(), None);
    }

    #[test]
    fn test_partial_success() {
        let items = vec![CompileArtifact::new(
            "doc-1",
            "A",
            DocumentFormat::Markdown,
            None,
            None,
        )];
        let failed = vec![FailedItem::new("missing.pdf", "File not found")];
        let result = CompileOutput::with_partial(items, failed);

        assert_eq!(result.len(), 1);
        assert!(result.has_failures());
        assert_eq!(result.total(), 2);
        assert_eq!(result.failed[0].source, "missing.pdf");
    }
}
