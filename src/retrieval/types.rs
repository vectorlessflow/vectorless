// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Core types for the retrieval system.

use serde::{Deserialize, Serialize};

use super::context::{PruningStrategy, TokenEstimation};
use crate::domain::NodeId;

/// Query complexity level for adaptive strategy selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryComplexity {
    /// Simple queries that can be solved with keyword matching.
    Simple,

    /// Medium complexity queries requiring semantic understanding.
    Medium,

    /// Complex queries requiring deep LLM reasoning.
    Complex,
}

impl Default for QueryComplexity {
    fn default() -> Self {
        Self::Medium
    }
}

/// Strategy preference for retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrategyPreference {
    /// Automatically select strategy based on query complexity.
    Auto,

    /// Force keyword-based strategy (fast, no LLM).
    ForceKeyword,

    /// Force semantic strategy (embedding-based).
    ForceSemantic,

    /// Force LLM strategy (deep reasoning).
    ForceLlm,
}

impl Default for StrategyPreference {
    fn default() -> Self {
        Self::Auto
    }
}

/// Sufficiency level for incremental retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SufficiencyLevel {
    /// Information is sufficient, stop retrieving.
    Sufficient,

    /// Partial information, can continue if needed.
    PartialSufficient,

    /// Information is insufficient, continue retrieving.
    Insufficient,
}

impl Default for SufficiencyLevel {
    fn default() -> Self {
        Self::Insufficient
    }
}

/// Options for retrieval operations.
#[derive(Debug, Clone)]
pub struct RetrieveOptions {
    /// Maximum number of results to return.
    pub top_k: usize,

    /// Beam width for multi-path search.
    pub beam_width: usize,

    /// Maximum search iterations.
    pub max_iterations: usize,

    /// Whether to include node content in results.
    pub include_content: bool,

    /// Whether to include node summaries in results.
    pub include_summaries: bool,

    /// Minimum relevance score (0.0 - 1.0).
    pub min_score: f32,

    /// Strategy preference.
    pub strategy: StrategyPreference,

    /// Enable sufficiency checking for incremental retrieval.
    pub sufficiency_check: bool,

    /// Maximum tokens for sufficiency threshold.
    pub max_tokens: usize,

    /// Enable result caching.
    pub enable_cache: bool,

    /// Pruning strategy for context building.
    pub pruning_strategy: super::PruningStrategy,

    /// Token estimation mode.
    pub token_estimation: super::TokenEstimation,

    /// Whether to use async context building for large documents.
    pub use_async_context: bool,
}

impl Default for RetrieveOptions {
    fn default() -> Self {
        Self {
            top_k: 5,
            beam_width: 3,
            max_iterations: 10,
            include_content: true,
            include_summaries: true,
            min_score: 0.1,
            strategy: StrategyPreference::Auto,
            sufficiency_check: true,
            max_tokens: 4000,
            enable_cache: true,
            pruning_strategy: super::PruningStrategy::default(),
            token_estimation: super::TokenEstimation::default(),
            use_async_context: false,
        }
    }
}

impl RetrieveOptions {
    /// Create new retrieve options with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of results to return.
    #[must_use]
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k;
        self
    }

    /// Set the beam width for multi-path search.
    #[must_use]
    pub fn with_beam_width(mut self, beam_width: usize) -> Self {
        self.beam_width = beam_width;
        self
    }

    /// Set the maximum search iterations.
    #[must_use]
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set whether to include node content in results.
    #[must_use]
    pub fn with_include_content(mut self, include: bool) -> Self {
        self.include_content = include;
        self
    }

    /// Set whether to include node summaries in results.
    #[must_use]
    pub fn with_include_summaries(mut self, include: bool) -> Self {
        self.include_summaries = include;
        self
    }

    /// Set the minimum relevance score.
    #[must_use]
    pub fn with_min_score(mut self, min_score: f32) -> Self {
        self.min_score = min_score;
        self
    }

    /// Set the strategy preference.
    #[must_use]
    pub fn with_strategy(mut self, strategy: StrategyPreference) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set whether to enable sufficiency checking.
    #[must_use]
    pub fn with_sufficiency_check(mut self, enable: bool) -> Self {
        self.sufficiency_check = enable;
        self
    }

    /// Set the maximum tokens for sufficiency threshold.
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set whether to enable result caching.
    #[must_use]
    pub fn with_enable_cache(mut self, enable: bool) -> Self {
        self.enable_cache = enable;
        self
    }

    /// Set pruning strategy for context building.
    #[must_use]
    pub fn with_pruning_strategy(mut self, strategy: PruningStrategy) -> Self {
        self.pruning_strategy = strategy;
        self
    }

    /// Set token estimation mode.
    #[must_use]
    pub fn with_token_estimation(mut self, mode: TokenEstimation) -> Self {
        self.token_estimation = mode;
        self
    }

    /// Enable async context building for large documents.
    #[must_use]
    pub fn with_async_context(mut self, enable: bool) -> Self {
        self.use_async_context = enable;
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

    /// Detected query complexity.
    pub complexity: QueryComplexity,

    /// Search trace for debugging.
    pub trace: Vec<NavigationStep>,

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
            complexity: QueryComplexity::Medium,
            trace: Vec::new(),
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

/// A single navigation step in the search trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationStep {
    /// Node ID visited.
    pub node_id: String,

    /// Node title.
    pub title: String,

    /// Relevance score at this step.
    pub score: f32,

    /// Decision made at this step.
    pub decision: NavigationDecision,

    /// Depth in tree.
    pub depth: usize,
}

/// Navigation decision at each step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavigationDecision {
    /// Go to the specified child.
    GoToChild(usize),

    /// This node contains the answer.
    ThisIsTheAnswer,

    /// Explore multiple children.
    ExploreMore,

    /// Skip this branch.
    Skip,
}

/// Search path for multi-path algorithms.
#[derive(Debug, Clone)]
pub struct SearchPath {
    /// Nodes in the path.
    pub nodes: Vec<NodeId>,

    /// Cumulative score.
    pub score: f32,

    /// Leaf node (if path ends at leaf).
    pub leaf: Option<NodeId>,
}

impl SearchPath {
    /// Create a new empty path.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            score: 0.0,
            leaf: None,
        }
    }

    /// Create a path from a single node.
    #[must_use]
    pub fn from_node(node_id: NodeId, score: f32) -> Self {
        Self {
            nodes: vec![node_id],
            score,
            leaf: Some(node_id),
        }
    }

    /// Extend the path with a new node.
    #[must_use]
    pub fn extend(&self, node_id: NodeId, score: f32) -> Self {
        let mut nodes = self.nodes.clone();
        nodes.push(node_id);
        Self {
            nodes,
            score: self.score + score,
            leaf: Some(node_id),
        }
    }
}

impl Default for SearchPath {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for a retrieval operation.
#[derive(Debug, Clone, Default)]
pub struct RetrievalStats {
    /// Number of nodes visited.
    pub nodes_visited: usize,

    /// Number of LLM calls made.
    pub llm_calls: usize,

    /// Time spent in milliseconds.
    pub time_ms: u64,

    /// Tokens consumed.
    pub tokens_used: usize,

    /// Cache hits.
    pub cache_hits: usize,

    /// Cache misses.
    pub cache_misses: usize,
}
