// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retriever types and options.

use serde::{Deserialize, Serialize};

/// Options for retrieval operations.
#[derive(Debug, Clone)]
pub struct RetrieveOptions {
    /// Maximum number of results to return.
    pub top_k: usize,

    /// Maximum tokens for the context window.
    pub max_context_tokens: usize,

    /// Whether to include node content in results.
    pub include_content: bool,

    /// Whether to include node summaries in results.
    pub include_summaries: bool,

    /// Minimum relevance score (0.0 - 1.0).
    pub min_score: f32,
}

impl Default for RetrieveOptions {
    fn default() -> Self {
        Self {
            top_k: 3,
            max_context_tokens: 4000,
            include_content: true,
            include_summaries: true,
            min_score: 0.0,
        }
    }
}

impl RetrieveOptions {
    /// Create new retrieve options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of results.
    pub fn with_top_k(mut self, k: usize) -> Self {
        self.top_k = k;
        self
    }

    /// Set the maximum context tokens.
    pub fn with_max_context_tokens(mut self, tokens: usize) -> Self {
        self.max_context_tokens = tokens;
        self
    }

    /// Set whether to include content.
    pub fn with_content(mut self, include: bool) -> Self {
        self.include_content = include;
        self
    }

    /// Set whether to include summaries.
    pub fn with_summaries(mut self, include: bool) -> Self {
        self.include_summaries = include;
        self
    }

    /// Set the minimum relevance score.
    pub fn with_min_score(mut self, score: f32) -> Self {
        self.min_score = score;
        self
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

    /// Relevance score.
    pub score: f32,

    /// Depth in the tree.
    pub depth: usize,

    /// Page range (for PDFs).
    pub page_range: Option<(usize, usize)>,
}

impl RetrievalResult {
    /// Create a new retrieval result.
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
    pub fn with_node_id(mut self, id: impl Into<String>) -> Self {
        self.node_id = Some(id.into());
        self
    }

    /// Set the content.
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Set the summary.
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Set the score.
    pub fn with_score(mut self, score: f32) -> Self {
        self.score = score;
        self
    }

    /// Set the depth.
    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// Set the page range.
    pub fn with_page_range(mut self, start: usize, end: usize) -> Self {
        self.page_range = Some((start, end));
        self
    }
}

/// Navigation decision for tree traversal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationDecision {
    /// Go to the specified child node.
    GoToChild(usize),

    /// The current node is the answer.
    ThisIsTheAnswer,

    /// Need to explore more at this level.
    ExploreMore,
}

/// Context for LLM-based navigation.
#[derive(Debug, Clone)]
pub struct NavigationContext {
    /// The user's query.
    pub query: String,

    /// Current path in the tree.
    pub path: Vec<String>,

    /// Available child summaries.
    pub child_summaries: Vec<String>,

    /// Current depth.
    pub depth: usize,

    /// Maximum depth to explore.
    pub max_depth: usize,
}

impl NavigationContext {
    /// Create a new navigation context.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            path: Vec::new(),
            child_summaries: Vec::new(),
            depth: 0,
            max_depth: 10,
        }
    }
}
