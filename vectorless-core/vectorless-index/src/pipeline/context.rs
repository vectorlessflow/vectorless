// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index context for passing data between stages.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::parse::{DocumentFormat, RawNode};
use vectorless_document::{Concept, DocumentTree, NavigationIndex, NodeId, ReasoningIndex};
use vectorless_llm::LlmClient;

use super::super::{PipelineOptions, SummaryStrategy};
use super::metrics::IndexMetrics;

/// Input for the index pipeline.
#[derive(Debug, Clone)]
pub enum IndexInput {
    /// Index from file path.
    File(PathBuf),

    /// Index from raw content string.
    Content {
        /// Content string.
        content: String,
        /// Document name.
        name: String,
        /// Document format.
        format: DocumentFormat,
    },

    /// Index from binary data.
    Bytes {
        /// Binary data.
        data: Vec<u8>,
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

    /// Create input from content string.
    pub fn content(content: impl Into<String>) -> Self {
        Self::Content {
            content: content.into(),
            name: String::new(),
            format: DocumentFormat::Markdown,
        }
    }

    /// Create input from content with name and format.
    pub fn content_with(
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

    /// Create input from binary data.
    pub fn bytes(data: impl Into<Vec<u8>>) -> Self {
        Self::Bytes {
            data: data.into(),
            name: String::new(),
            format: DocumentFormat::Pdf,
        }
    }

    /// Create input from binary data with name and format.
    pub fn bytes_with(
        data: impl Into<Vec<u8>>,
        name: impl Into<String>,
        format: DocumentFormat,
    ) -> Self {
        Self::Bytes {
            data: data.into(),
            name: name.into(),
            format,
        }
    }

    /// Check if this is a file input.
    pub fn is_file(&self) -> bool {
        matches!(self, Self::File(_))
    }

    /// Check if this is a content input.
    pub fn is_content(&self) -> bool {
        matches!(self, Self::Content { .. })
    }

    /// Check if this is a bytes input.
    pub fn is_bytes(&self) -> bool {
        matches!(self, Self::Bytes { .. })
    }

    /// Get the format if available.
    pub fn format(&self) -> Option<DocumentFormat> {
        match self {
            Self::File(_) => None,
            Self::Content { format, .. } => Some(*format),
            Self::Bytes { format, .. } => Some(*format),
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

    /// SHA-256 hash of source content for checkpoint validation.
    pub source_hash: String,

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

    /// Pre-computed reasoning index (built by ReasoningIndexStage).
    pub reasoning_index: Option<ReasoningIndex>,

    /// Navigation index for Agent-based retrieval (built by NavigationIndexStage).
    pub navigation_index: Option<NavigationIndex>,

    /// Key concepts extracted from the document (built by ConceptExtractionStage).
    pub concepts: Vec<Concept>,

    /// Existing tree from previous indexing (for incremental updates).
    /// When set, the enhance and reasoning stages can reuse data from unchanged nodes.
    pub existing_tree: Option<DocumentTree>,

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
        let source_hash = Self::compute_source_hash(&input);
        Self {
            doc_id: uuid::Uuid::new_v4().to_string(),
            input,
            format: DocumentFormat::Markdown,
            name: String::new(),
            source_path: None,
            source_hash,
            raw_nodes: Vec::new(),
            tree: None,
            options,
            llm_client: None,
            summary_cache: SummaryCache::default(),
            reasoning_index: None,
            navigation_index: None,
            concepts: Vec::new(),
            existing_tree: None,
            stage_results: HashMap::new(),
            metrics: IndexMetrics::default(),
            description: None,
            page_count: None,
            line_count: None,
        }
    }

    /// Compute SHA-256 hash of the source content.
    fn compute_source_hash(input: &IndexInput) -> String {
        use sha2::{Digest, Sha256};
        let hash = match input {
            IndexInput::File(path) => {
                // Hash the file path as proxy — actual content may not be readable yet
                // (the parse stage reads it). This is sufficient for checkpoint invalidation
                // since a different file path implies different content.
                Sha256::digest(path.to_string_lossy().as_bytes())
            }
            IndexInput::Content { content, .. } => Sha256::digest(content.as_bytes()),
            IndexInput::Bytes { data, .. } => Sha256::digest(data),
        };
        format!("{:x}", hash)
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

    /// Set the existing tree for incremental updates.
    pub fn with_existing_tree(mut self, tree: DocumentTree) -> Self {
        self.existing_tree = Some(tree);
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
    pub fn finalize(self) -> PipelineResult {
        PipelineResult {
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
            reasoning_index: self.reasoning_index,
            navigation_index: self.navigation_index,
            concepts: self.concepts,
        }
    }
}

/// Final result from the index pipeline.
#[derive(Debug)]
pub struct PipelineResult {
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

    /// Pre-computed reasoning index for retrieval acceleration.
    pub reasoning_index: Option<ReasoningIndex>,

    /// Navigation index for Agent-based retrieval.
    pub navigation_index: Option<NavigationIndex>,

    /// Key concepts extracted from the document.
    pub concepts: Vec<Concept>,
}

impl PipelineResult {
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
            + self.metrics.validate_time_ms
            + self.metrics.split_time_ms
            + self.metrics.enhance_time_ms
            + self.metrics.enrich_time_ms
            + self.metrics.reasoning_index_time_ms
            + self.metrics.navigation_index_time_ms
            + self.metrics.optimize_time_ms
    }
}
