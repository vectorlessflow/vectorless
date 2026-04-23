// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Core types for the retrieval system.

use serde::{Deserialize, Serialize};

/// Re-export [`SufficiencyLevel`] from the document module.
pub use vectorless_document::SufficiencyLevel;

/// Complete retrieval response.
#[derive(Debug, Clone)]
pub struct RetrieveResponse {
    /// Retrieved results.
    pub results: Vec<RetrievalResult>,

    /// Aggregated content.
    pub content: String,

    /// Overall confidence score.
    pub confidence: f32,

    /// Whether information is sufficient.
    pub is_sufficient: bool,

    /// Strategy that was used.
    pub strategy_used: String,

    /// Reasoning chain explaining how results were found.
    pub reasoning_chain: ReasoningChain,

    /// Total tokens used.
    pub tokens_used: usize,
}

impl Default for RetrieveResponse {
    fn default() -> Self {
        Self {
            results: Vec::new(),
            content: String::new(),
            confidence: 0.0,
            is_sufficient: false,
            strategy_used: String::new(),
            reasoning_chain: ReasoningChain::default(),
            tokens_used: 0,
        }
    }
}

impl RetrieveResponse {
    /// Create a new empty response.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are any results.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Get the number of results.
    #[must_use]
    pub fn len(&self) -> usize {
        self.results.len()
    }
}

/// A single retrieval result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalResult {
    /// Node ID in the tree.
    pub node_id: Option<String>,

    /// Node title.
    pub title: String,

    /// Node content (if included).
    pub content: Option<String>,

    /// Node summary (if included).
    pub summary: Option<String>,

    /// Relevance score (0.0 - 1.0).
    pub score: f32,

    /// Depth in the tree.
    pub depth: usize,

    /// Page range (for PDFs).
    pub page_range: Option<(usize, usize)>,
}

impl RetrievalResult {
    /// Create a new retrieval result.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            node_id: None,
            title: title.into(),
            content: None,
            summary: None,
            score: 1.0,
            depth: 0,
            page_range: None,
        }
    }

    /// Set the node ID.
    #[must_use]
    pub fn with_node_id(mut self, id: impl Into<String>) -> Self {
        self.node_id = Some(id.into());
        self
    }

    /// Set the content.
    #[must_use]
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Set the summary.
    #[must_use]
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Set the score.
    #[must_use]
    pub fn with_score(mut self, score: f32) -> Self {
        self.score = score;
        self
    }

    /// Set the depth.
    #[must_use]
    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// Set the page range.
    #[must_use]
    pub fn with_page_range(mut self, start: usize, end: usize) -> Self {
        self.page_range = Some((start, end));
        self
    }
}

/// Complete reasoning chain for a retrieval operation.
///
/// Provides an ordered, auditable trace of every decision the engine made
/// from query analysis through final evaluation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReasoningChain {
    /// Ordered reasoning steps.
    pub steps: Vec<ReasoningStep>,
}

impl ReasoningChain {
    /// Create an empty reasoning chain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a reasoning step.
    pub fn push(&mut self, step: ReasoningStep) {
        self.steps.push(step);
    }

    /// Number of reasoning steps.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

/// A single step in the reasoning chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    /// Human-readable explanation of the decision.
    pub reasoning: String,
}

// ============================================================
// Query result types (used by engine)
// ============================================================

/// Confidence score of the query result (0.0–1.0).
pub type Confidence = f32;

/// A single piece of evidence with source attribution.
#[derive(Debug, Clone)]
pub struct EvidenceItem {
    /// Section title where this evidence was found.
    pub title: String,
    /// Navigation path (e.g., "Root/Chapter 1/Section 1.2").
    pub path: String,
    /// Raw evidence content.
    pub content: String,
    /// Source document name (set in multi-doc scenarios).
    pub doc_name: Option<String>,
}

/// Query execution metrics.
#[derive(Debug, Clone, Default)]
pub struct QueryMetrics {
    /// Number of LLM calls made.
    pub llm_calls: u32,
    /// Number of navigation rounds used.
    pub rounds_used: u32,
    /// Number of distinct nodes visited.
    pub nodes_visited: usize,
    /// Number of evidence items collected.
    pub evidence_count: usize,
    /// Total characters of collected evidence.
    pub evidence_chars: usize,
}

/// A single document's query result.
#[derive(Debug, Clone)]
pub struct QueryResultItem {
    /// The document ID.
    pub doc_id: String,
    /// Matching node IDs (navigation paths).
    pub node_ids: Vec<String>,
    /// Synthesized answer or raw evidence content.
    pub content: String,
    /// Evidence items that contributed to this result, with source attribution.
    pub evidence: Vec<EvidenceItem>,
    /// Execution metrics for this query.
    pub metrics: Option<QueryMetrics>,
    /// Confidence score (0.0–1.0) — derived from LLM evaluation.
    pub confidence: Confidence,
}
