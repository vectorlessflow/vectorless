// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index context for passing data between stages.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::document::{DocumentTree, NodeId};
use crate::llm::LlmClient;
use crate::parser::{DocumentFormat, RawNode};

use super::super::{PipelineOptions, SummaryStrategy};
use super::metrics::IndexMetrics;

/// Input for the index pipeline.
#[derive(Debug, Clone)]
pub enum IndexInput {
    /// Index from file path.
    File(PathBuf),

    /// Index from raw content.
    Content {
        /// Content string.
        content: String,
        /// Document name.
        name: String,
        /// Document format.
        format: DocumentFormat,
    },
}

impl IndexInput {
    /// Create input from file path.
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File(path.into())
    }

    /// Create input from content.
    pub fn content(
        content: impl Into<String>,
        name: impl Into<String>,
        format: DocumentFormat,
    ) -> Self {
        Self::Content {
            content: content.into(),
            name: name.into(),
            format,
        }
    }
}

/// Result from a single stage execution.
#[derive(Debug, Clone)]
pub struct StageResult {
    /// Whether the stage succeeded.
    pub success: bool,

    /// Duration in milliseconds.
    pub duration_ms: u64,

    /// Additional metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl StageResult {
    /// Create a successful result.
    pub fn success(name: &str) -> Self {
        println!("Stage '{}' completed successfully", name);

        Self {
            success: true,
            duration_ms: 0,
            metadata: HashMap::new(),
        }
    }

    /// Create a failed result.
    pub fn failure(name: &str, error: &str) -> Self {
        println!("Stage '{}' failed: {}", name, error);

        let mut metadata = HashMap::new();
        metadata.insert(
            "error".to_string(),
            serde_json::Value::String(error.to_string()),
        );
        Self {
            success: false,
            duration_ms: 0,
            metadata,
        }
    }

    /// Set duration.
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }
}

/// Summary cache for lazy generation.
#[derive(Debug, Clone, Default)]
pub struct SummaryCache {
    /// Cached summaries: node_id -> summary.
    summaries: HashMap<NodeId, String>,

    /// Whether to persist to disk.
    persist: bool,
}

impl SummaryCache {
    /// Create a new cache.
    pub fn new(persist: bool) -> Self {
        Self {
            summaries: HashMap::new(),
            persist,
        }
    }

    /// Get a cached summary.
    pub fn get(&self, node_id: NodeId) -> Option<&str> {
        self.summaries.get(&node_id).map(|s| s.as_str())
    }

    /// Store a summary.
    pub fn put(&mut self, node_id: NodeId, summary: String) {
        self.summaries.insert(node_id, summary);
    }

    /// Whether persistence is enabled.
    pub fn should_persist(&self) -> bool {
        self.persist
    }

    /// Get all cached summaries.
    pub fn all(&self) -> &HashMap<NodeId, String> {
        &self.summaries
    }
}

/// Index context passed between stages.
#[derive(Debug)]
pub struct IndexContext {
    /// Document ID.
    pub doc_id: String,

    /// Source input.
    pub input: IndexInput,

    /// Document format.
    pub format: DocumentFormat,

    /// Document name.
    pub name: String,

    /// Source file path (if from file).
    pub source_path: Option<PathBuf>,

    /// Parsed raw nodes.
    pub raw_nodes: Vec<RawNode>,

    /// Built document tree.
    pub tree: Option<DocumentTree>,

    /// Index options.
    pub options: PipelineOptions,

    /// LLM client for enhancement.
    pub llm_client: Option<LlmClient>,

    /// Summary cache for lazy generation.
    pub summary_cache: SummaryCache,

    /// Stage execution results.
    pub stage_results: HashMap<String, StageResult>,

    /// Performance metrics.
    pub metrics: IndexMetrics,

    /// Document description.
    pub description: Option<String>,

    /// Page count (for PDFs).
    pub page_count: Option<usize>,

    /// Line count.
    pub line_count: Option<usize>,
}

impl IndexContext {
    /// Create a new context from input.
    pub fn new(input: IndexInput, options: PipelineOptions) -> Self {
        Self {
            doc_id: uuid::Uuid::new_v4().to_string(),
            input,
            format: DocumentFormat::Markdown,
            name: String::new(),
            source_path: None,
            raw_nodes: Vec::new(),
            tree: None,
            options,
            llm_client: None,
            summary_cache: SummaryCache::default(),
            stage_results: HashMap::new(),
            metrics: IndexMetrics::default(),
            description: None,
            page_count: None,
            line_count: None,
        }
    }

    /// Set the document ID.
    pub fn with_doc_id(mut self, doc_id: impl Into<String>) -> Self {
        self.doc_id = doc_id.into();
        self
    }

    /// Set the LLM client.
    pub fn with_llm_client(mut self, client: LlmClient) -> Self {
        self.llm_client = Some(client);
        self
    }

    /// Set the document format.
    pub fn with_format(mut self, format: DocumentFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the document name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the source path.
    pub fn with_source_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    /// Initialize summary cache based on strategy.
    pub fn init_summary_cache(&mut self) {
        if let SummaryStrategy::Lazy { persist, .. } = self.options.summary_strategy {
            self.summary_cache = SummaryCache::new(persist);
        }
    }

    /// Record a stage result.
    pub fn record_stage(&mut self, name: &str, result: StageResult) {
        self.stage_results.insert(name.to_string(), result);
    }

    /// Get the tree, returning an error if not built.
    pub fn tree(&self) -> Result<&DocumentTree, &'static str> {
        self.tree.as_ref().ok_or("Tree not built")
    }

    /// Get mutable tree, returning an error if not built.
    pub fn tree_mut(&mut self) -> Result<&mut DocumentTree, &'static str> {
        self.tree.as_mut().ok_or("Tree not built")
    }

    /// Finalize and build the result.
    pub fn finalize(self) -> IndexResult {
        IndexResult {
            doc_id: self.doc_id,
            name: self.name,
            format: self.format,
            source_path: self.source_path,
            tree: self.tree,
            description: self.description,
            page_count: self.page_count,
            line_count: self.line_count,
            metrics: self.metrics,
            summary_cache: self.summary_cache,
        }
    }
}

/// Final result from the index pipeline.
#[derive(Debug)]
pub struct IndexResult {
    /// Document ID.
    pub doc_id: String,

    /// Document name.
    pub name: String,

    /// Document format.
    pub format: DocumentFormat,

    /// Source file path.
    pub source_path: Option<PathBuf>,

    /// Built document tree.
    pub tree: Option<DocumentTree>,

    /// Document description.
    pub description: Option<String>,

    /// Page count (for PDFs).
    pub page_count: Option<usize>,

    /// Line count.
    pub line_count: Option<usize>,

    /// Performance metrics.
    pub metrics: IndexMetrics,

    /// Summary cache.
    pub summary_cache: SummaryCache,
}

impl IndexResult {
    /// Check if the result has a tree.
    pub fn has_tree(&self) -> bool {
        self.tree.is_some()
    }

    /// Get the tree.
    pub fn tree(&self) -> Option<&DocumentTree> {
        self.tree.as_ref()
    }

    /// Get total indexing time in milliseconds.
    pub fn total_time_ms(&self) -> u64 {
        self.metrics.parse_time_ms
            + self.metrics.build_time_ms
            + self.metrics.enhance_time_ms
            + self.metrics.enrich_time_ms
            + self.metrics.optimize_time_ms
            + self.metrics.persist_time_ms
    }
}
