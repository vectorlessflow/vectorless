// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Public API types for the client module.
//!
//! This module contains all types exposed in the public API.

use serde::{Deserialize, Serialize};

use vectorless_document::DocumentFormat;
use vectorless_metrics::IndexMetrics;

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
// Index Types
// ============================================================

/// Document indexing behavior mode.
///
/// Controls how the indexer handles existing documents and re-indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndexMode {
    /// Default mode - skip if already indexed.
    ///
    /// If a document with the same source has already been indexed,
    /// the operation is skipped and the existing document ID is returned.
    #[default]
    Default,

    /// Force re-indexing.
    ///
    /// Always re-index the document, even if it has been indexed before.
    /// A new document ID is generated.
    Force,

    /// Incremental mode - only re-index changed files.
    ///
    /// Re-index only if the file has been modified since the last index.
    /// For content/bytes sources, this behaves like [`IndexMode::Default`].
    Incremental,
}

/// Options for indexing a document.
#[derive(Debug, Clone)]
pub struct IndexOptions {
    /// Indexing mode.
    pub mode: IndexMode,

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

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            mode: IndexMode::Default,
            generate_summaries: true,
            generate_ids: true,
            generate_description: true,
            enable_synonym_expansion: true,
            timeout_secs: None,
        }
    }
}

impl IndexOptions {
    /// Create new index options with defaults.
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

    /// Set the indexing mode.
    ///
    /// # Modes
    ///
    /// - [`IndexMode::Default`] - Skip if already indexed
    /// - [`IndexMode::Force`] - Always re-index
    /// - [`IndexMode::Incremental`] - Only re-index changed files
    pub fn with_mode(mut self, mode: IndexMode) -> Self {
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
// Index Result Types
// ============================================================

/// Result of a document indexing operation.
#[derive(Debug, Clone)]
pub struct IndexResult {
    /// Successfully indexed items.
    pub items: Vec<IndexItem>,

    /// Items that failed to index (partial success).
    pub failed: Vec<FailedItem>,
}

impl IndexResult {
    /// Create a new index result.
    pub fn new(items: Vec<IndexItem>) -> Self {
        Self {
            items,
            failed: Vec::new(),
        }
    }

    /// Create with both successes and failures.
    pub fn with_partial(items: Vec<IndexItem>, failed: Vec<FailedItem>) -> Self {
        Self { items, failed }
    }

    /// Get the single document ID (convenience for single-document indexing).
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

    /// Get the number of indexed items.
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

/// A single indexed document item.
#[derive(Debug, Clone)]
pub struct IndexItem {
    /// The unique document ID.
    pub doc_id: String,
    /// The document name.
    pub name: String,
    /// The document format.
    pub format: DocumentFormat,
    /// Document description (from root summary).
    pub description: Option<String>,
    /// Source file path (if indexed from a file).
    pub source_path: Option<String>,
    /// Page count (for PDFs).
    pub page_count: Option<usize>,
    /// Indexing pipeline metrics (timing, LLM usage, node stats).
    pub metrics: Option<IndexMetrics>,
}

impl IndexItem {
    /// Create a new index item.
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

    /// Set the indexing metrics.
    pub fn with_metrics(mut self, metrics: IndexMetrics) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Set the indexing metrics (optional).
    pub fn with_metrics_opt(mut self, metrics: Option<IndexMetrics>) -> Self {
        self.metrics = metrics;
        self
    }
}

// ============================================================
// Query Types — defined locally (strategy layer moved to Python)
// ============================================================

/// Confidence level of a query result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Confidence(pub f64);

impl Confidence {
    /// Create a new confidence value (0.0 - 1.0).
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    /// Get the raw value.
    pub fn value(&self) -> f64 {
        self.0
    }
}

/// A piece of evidence supporting a query result.
#[derive(Debug, Clone)]
pub struct EvidenceItem {
    /// Title of the source section.
    pub title: String,
    /// Path within the document.
    pub path: String,
    /// Content of the evidence.
    pub content: String,
}

/// Metrics for a single query result.
#[derive(Debug, Clone, Default)]
pub struct QueryMetrics {
    /// Number of LLM calls made.
    pub llm_calls: usize,
    /// Number of navigation rounds used.
    pub rounds_used: usize,
    /// Number of document nodes visited.
    pub nodes_visited: usize,
    /// Number of evidence items collected.
    pub evidence_count: usize,
    /// Total characters in evidence.
    pub evidence_chars: usize,
}

/// A single query result item.
#[derive(Debug, Clone)]
pub struct QueryResultItem {
    /// Document ID.
    pub doc_id: String,
    /// Node IDs that contributed evidence.
    pub node_ids: Vec<String>,
    /// Result content.
    pub content: String,
    /// Supporting evidence.
    pub evidence: Vec<EvidenceItem>,
    /// Optional metrics.
    pub metrics: Option<QueryMetrics>,
    /// Confidence score.
    pub confidence: f64,
}

/// Result of a document query.
///
/// Contains results from one or more documents. For single-document queries,
/// `items` has one entry. For multi-document or workspace queries, it has
/// one entry per document that matched.
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// Query results per document.
    pub items: Vec<QueryResultItem>,

    /// Documents that failed during multi-doc query.
    pub failed: Vec<FailedItem>,
}

impl QueryResult {
    /// Create a new query result (empty).
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            failed: Vec::new(),
        }
    }

    /// Create a query result with items.
    pub fn new_with_items(items: Vec<QueryResultItem>) -> Self {
        Self {
            items,
            failed: Vec::new(),
        }
    }

    /// Create a query result with a single item.
    pub fn from_single(item: QueryResultItem) -> Self {
        Self {
            items: vec![item],
            failed: Vec::new(),
        }
    }

    /// Create with both successes and failures.
    pub fn with_partial(items: Vec<QueryResultItem>, failed: Vec<FailedItem>) -> Self {
        Self { items, failed }
    }

    /// Check if the result is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the number of result items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Get the first (single-doc) result item, if any.
    pub fn single(&self) -> Option<&QueryResultItem> {
        self.items.first()
    }

    /// Whether any documents failed.
    pub fn has_failures(&self) -> bool {
        !self.failed.is_empty()
    }
}

impl Default for QueryResult {
    fn default() -> Self {
        Self::new()
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

impl DocumentInfo {
    /// Create a new document info.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            format: String::new(),
            description: None,
            source_path: None,
            page_count: None,
            line_count: None,
        }
    }

    /// Set the format.
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = format.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_options() {
        let options = IndexOptions::new()
            .with_summaries()
            .with_mode(IndexMode::Force);

        assert!(options.generate_summaries);
        assert_eq!(options.mode, IndexMode::Force);
    }

    #[test]
    fn test_index_options_timeout() {
        let opts = IndexOptions::new().with_timeout_secs(30);
        assert_eq!(opts.timeout_secs, Some(30));

        let default = IndexOptions::default();
        assert_eq!(default.timeout_secs, None);
    }

    #[test]
    fn test_query_result() {
        let result = QueryResult::new();
        assert!(result.is_empty());
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_query_result_single() {
        let item = QueryResultItem {
            doc_id: "doc-1".into(),
            node_ids: vec!["n1".into()],
            content: "content".into(),
            evidence: vec![],
            metrics: None,
            confidence: 0.9,
        };
        let result = QueryResult::from_single(item);
        assert!(!result.is_empty());
        assert_eq!(result.len(), 1);
        assert!(result.single().is_some());
        assert_eq!(result.single().unwrap().doc_id, "doc-1");
    }

    #[test]
    fn test_document_info() {
        let info = DocumentInfo::new("doc-1", "Test").with_format("markdown");

        assert_eq!(info.id, "doc-1");
        assert_eq!(info.format, "markdown");
    }

    #[test]
    fn test_index_result() {
        let item = IndexItem::new("doc-1", "Test", DocumentFormat::Markdown, None, None);
        let result = IndexResult::new(vec![item]);

        assert_eq!(result.doc_id(), Some("doc-1"));
        assert_eq!(result.len(), 1);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_index_result_empty() {
        let result = IndexResult::new(vec![]);
        assert!(result.is_empty());
        assert_eq!(result.doc_id(), None);
    }

    #[test]
    fn test_index_result_multiple() {
        let items = vec![
            IndexItem::new("doc-1", "A", DocumentFormat::Markdown, None, None),
            IndexItem::new("doc-2", "B", DocumentFormat::Pdf, None, None),
        ];
        let result = IndexResult::new(items);
        assert_eq!(result.len(), 2);
        assert_eq!(result.doc_id(), None);
    }

    #[test]
    fn test_partial_success() {
        let items = vec![IndexItem::new(
            "doc-1",
            "A",
            DocumentFormat::Markdown,
            None,
            None,
        )];
        let failed = vec![FailedItem::new("missing.pdf", "File not found")];
        let result = IndexResult::with_partial(items, failed);

        assert_eq!(result.len(), 1);
        assert!(result.has_failures());
        assert_eq!(result.total(), 2);
        assert_eq!(result.failed[0].source, "missing.pdf");
    }
}
