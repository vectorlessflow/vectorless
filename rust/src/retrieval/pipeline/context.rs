// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval pipeline context.
//!
//! Context passed between retrieval stages, accumulating data throughout
//! the retrieval process.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::document::{DocumentTree, NodeId, ReasoningIndex, RetrievalIndex};
use crate::graph::DocumentGraph;
use crate::retrieval::cache::{HotNodeTracker, ReasoningCache};
use crate::retrieval::pilot::Pilot;
use crate::retrieval::pipeline::budget::RetrievalBudgetController;
use crate::retrieval::types::{
    NavigationDecision, QueryComplexity, ReasoningChain, ReasoningStep, RetrieveOptions,
    RetrieveResponse, SearchPath, StageName, StrategyPreference, SufficiencyLevel,
};

/// Search algorithm type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchAlgorithm {
    /// Greedy single-path search.
    Greedy,
    /// Beam search with multiple paths.
    Beam,
    /// Monte Carlo Tree Search.
    Mcts,
}

impl Default for SearchAlgorithm {
    fn default() -> Self {
        Self::Beam
    }
}

impl SearchAlgorithm {
    /// Get algorithm name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Greedy => "greedy",
            Self::Beam => "beam",
            Self::Mcts => "mcts",
        }
    }
}

/// Search configuration.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Beam width for multi-path search.
    pub beam_width: usize,
    /// Maximum depth to search.
    pub max_depth: usize,
    /// Minimum relevance score.
    pub min_score: f32,
    /// Maximum search iterations.
    pub max_iterations: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            beam_width: 3,
            max_depth: 10,
            min_score: 0.1,
            max_iterations: 5,
        }
    }
}

/// Candidate node from search.
#[derive(Debug, Clone)]
pub struct CandidateNode {
    /// Node ID in the tree.
    pub node_id: NodeId,
    /// Relevance score (0.0 - 1.0).
    pub score: f32,
    /// Depth in the tree.
    pub depth: usize,
    /// Whether this is a leaf node.
    pub is_leaf: bool,
}

impl CandidateNode {
    /// Create a new candidate node.
    pub fn new(node_id: NodeId, score: f32, depth: usize, is_leaf: bool) -> Self {
        Self {
            node_id,
            score,
            depth,
            is_leaf,
        }
    }
}

/// Stage execution result.
#[derive(Debug, Clone)]
pub struct StageResult {
    /// Stage name.
    pub stage: String,
    /// Whether successful.
    pub success: bool,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Optional message.
    pub message: Option<String>,
}

impl StageResult {
    /// Create a successful result.
    pub fn success(stage: impl Into<String>) -> Self {
        Self {
            stage: stage.into(),
            success: true,
            duration_ms: 0,
            message: None,
        }
    }

    /// Create a failed result.
    pub fn failure(stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            stage: stage.into(),
            success: false,
            duration_ms: 0,
            message: Some(message.into()),
        }
    }

    /// Set duration.
    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }
}

/// Retrieval performance metrics.
#[derive(Debug, Clone, Default)]
pub struct RetrievalMetrics {
    /// Time spent in analyze stage (ms).
    pub analyze_time_ms: u64,
    /// Time spent in plan stage (ms).
    pub plan_time_ms: u64,
    /// Time spent in search stage (ms).
    pub search_time_ms: u64,
    /// Time spent in evaluate stage (ms).
    pub evaluate_time_ms: u64,
    /// Total time (ms).
    pub total_time_ms: u64,
    /// Number of nodes visited.
    pub nodes_visited: usize,
    /// Number of LLM calls.
    pub llm_calls: usize,
    /// Tokens consumed.
    pub tokens_used: usize,
    /// Cache hits.
    pub cache_hits: usize,
    /// Cache misses.
    pub cache_misses: usize,
    /// Search iterations performed.
    pub search_iterations: usize,
    /// Backtrack count.
    pub backtracks: usize,
}

impl RetrievalMetrics {
    /// Create new metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge another metrics into this one.
    pub fn merge(&mut self, other: &RetrievalMetrics) {
        self.analyze_time_ms += other.analyze_time_ms;
        self.plan_time_ms += other.plan_time_ms;
        self.search_time_ms += other.search_time_ms;
        self.evaluate_time_ms += other.evaluate_time_ms;
        self.nodes_visited += other.nodes_visited;
        self.llm_calls += other.llm_calls;
        self.tokens_used = other.tokens_used; // Use latest
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        self.search_iterations = other.search_iterations; // Use latest
        self.backtracks += other.backtracks;
    }
}

/// Retrieval pipeline context.
///
/// Passed between stages and accumulates data throughout the retrieval process.
pub struct PipelineContext {
    // ============ Input ============
    /// Original query string.
    pub query: String,
    /// Document tree to search.
    pub tree: Arc<DocumentTree>,
    /// Pre-computed retrieval index for efficient operations.
    pub retrieval_index: Option<RetrievalIndex>,
    /// Retrieval options.
    pub options: RetrieveOptions,
    /// Optional Pilot for navigation guidance.
    pub pilot: Option<Arc<dyn Pilot>>,
    /// Adaptive token budget controller for the entire pipeline.
    /// Shared via Arc so Pilot can read/check the same budget.
    pub budget_controller: Arc<RetrievalBudgetController>,
    /// Tiered reasoning cache (L1 exact, L2 path pattern, L3 strategy score).
    pub reasoning_cache: Arc<ReasoningCache>,

    /// Pre-computed reasoning index for fast path resolution.
    pub reasoning_index: Option<Arc<ReasoningIndex>>,

    /// Hot node tracker for recording retrieval frequency (session-scoped).
    pub hot_tracker: Option<Arc<HotNodeTracker>>,

    /// Cross-document relationship graph for graph-aware retrieval.
    pub document_graph: Option<Arc<DocumentGraph>>,

    // ============ Analyze Stage Output ============
    /// Detected query complexity.
    pub complexity: Option<QueryComplexity>,
    /// Extracted keywords.
    pub keywords: Vec<String>,
    /// Target sections from ToC matching.
    pub target_sections: Vec<String>,
    /// Resolved structural path hints — node IDs extracted from the query
    /// (e.g. "第3章" → NodeId of Chapter 3). Search should start from these nodes.
    pub resolved_path_hints: Vec<(String, NodeId)>,
    /// Decomposed sub-queries (if query was decomposed).
    pub decomposition: Option<crate::retrieval::decompose::DecompositionResult>,

    // ============ Plan Stage Output ============
    /// Selected retrieval strategy.
    pub selected_strategy: Option<StrategyPreference>,
    /// Selected search algorithm.
    pub selected_algorithm: Option<SearchAlgorithm>,
    /// Search configuration.
    pub search_config: Option<SearchConfig>,

    // ============ Search Stage Output ============
    /// Candidate nodes from search.
    pub candidates: Vec<CandidateNode>,
    /// Search paths explored.
    pub search_paths: Vec<SearchPath>,
    /// Reasoning chain — ordered steps explaining every retrieval decision.
    pub reasoning_chain: ReasoningChain,
    /// Number of search iterations performed.
    pub search_iterations: usize,

    // ============ Evaluate Stage Output ============
    /// Current sufficiency level.
    pub sufficiency: SufficiencyLevel,
    /// Accumulated content from candidates.
    pub accumulated_content: String,
    /// Estimated token count.
    pub token_count: usize,
    /// Fingerprint of candidate node IDs from previous evaluate call.
    /// Used to detect stagnant loops (same candidates → same evaluation).
    pub prev_candidate_fingerprint: Option<u64>,
    /// Per-node content cache to avoid duplicate computation.
    /// Populated by `aggregate_content()`, read by `build_response()`.
    pub node_content_cache: HashMap<NodeId, String>,

    // ============ Final Result ============
    /// Final retrieval response.
    pub result: Option<RetrieveResponse>,

    // ============ Metadata ============
    /// Stage execution results.
    pub stage_results: HashMap<String, StageResult>,
    /// Performance metrics.
    pub metrics: RetrievalMetrics,
    /// Start time of current stage.
    pub stage_start: Option<Instant>,
}

impl PipelineContext {
    /// Create a new retrieval context.
    pub fn new(
        tree: Arc<DocumentTree>,
        query: impl Into<String>,
        options: RetrieveOptions,
    ) -> Self {
        // Build retrieval index for efficient operations
        let retrieval_index = Some(tree.build_retrieval_index());
        let budget_controller = Arc::new(RetrievalBudgetController::new(options.max_tokens));

        Self {
            query: query.into(),
            tree,
            retrieval_index,
            options,
            pilot: None,
            budget_controller,
            reasoning_cache: Arc::new(ReasoningCache::new()),
            reasoning_index: None,
            hot_tracker: None,
            document_graph: None,
            complexity: None,
            keywords: Vec::new(),
            target_sections: Vec::new(),
            resolved_path_hints: Vec::new(),
            decomposition: None,
            selected_strategy: None,
            selected_algorithm: None,
            search_config: None,
            candidates: Vec::new(),
            search_paths: Vec::new(),
            reasoning_chain: ReasoningChain::new(),
            search_iterations: 0,
            sufficiency: SufficiencyLevel::default(),
            accumulated_content: String::new(),
            token_count: 0,
            prev_candidate_fingerprint: None,
            node_content_cache: HashMap::new(),
            result: None,
            stage_results: HashMap::new(),
            metrics: RetrievalMetrics::default(),
            stage_start: None,
        }
    }

    /// Create a new retrieval context with Pilot.
    pub fn with_pilot(
        tree: Arc<DocumentTree>,
        query: impl Into<String>,
        options: RetrieveOptions,
        pilot: Option<Arc<dyn Pilot>>,
    ) -> Self {
        let mut ctx = Self::new(tree, query, options);
        ctx.pilot = pilot;
        ctx
    }

    /// Set the Pilot for this context.
    pub fn set_pilot(&mut self, pilot: Option<Arc<dyn Pilot>>) {
        self.pilot = pilot;
    }

    /// Set the reasoning index for this retrieval context.
    pub fn with_reasoning_index(mut self, index: ReasoningIndex) -> Self {
        self.reasoning_index = Some(Arc::new(index));
        self
    }

    /// Set the hot node tracker for this retrieval context.
    pub fn with_hot_tracker(mut self, tracker: HotNodeTracker) -> Self {
        self.hot_tracker = Some(Arc::new(tracker));
        self
    }

    /// Set the document graph for graph-aware retrieval.
    pub fn with_document_graph(mut self, graph: Arc<DocumentGraph>) -> Self {
        self.document_graph = Some(graph);
        self
    }

    /// Get the Pilot reference, if available.
    pub fn pilot(&self) -> Option<&dyn Pilot> {
        self.pilot.as_deref()
    }

    /// Start timing a stage.
    pub fn start_stage(&mut self) {
        self.stage_start = Some(Instant::now());
    }

    /// End timing and record for a stage.
    pub fn end_stage(&mut self, stage_name: &str, success: bool, message: Option<String>) {
        let duration_ms = self
            .stage_start
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);

        let result = StageResult {
            stage: stage_name.to_string(),
            success,
            duration_ms,
            message,
        };

        // Update metrics based on stage
        match stage_name {
            "analyze" => self.metrics.analyze_time_ms += duration_ms,
            "plan" => self.metrics.plan_time_ms += duration_ms,
            "search" => self.metrics.search_time_ms += duration_ms,
            "evaluate" => self.metrics.evaluate_time_ms += duration_ms,
            _ => {}
        }

        self.stage_results.insert(stage_name.to_string(), result);
        self.stage_start = None;
    }

    /// Check if we can perform more search iterations.
    pub fn can_search_more(&self) -> bool {
        self.search_iterations < self.options.max_iterations
    }

    /// Increment search iteration count.
    pub fn increment_search_iteration(&mut self) {
        self.search_iterations += 1;
        self.metrics.search_iterations = self.search_iterations;
    }

    /// Increment backtrack count.
    pub fn increment_backtrack(&mut self) {
        self.metrics.backtracks += 1;
    }

    /// Compute a fingerprint of the current candidate node IDs.
    fn candidate_fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for c in &self.candidates {
            format!("{:?}", c.node_id).hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Check if candidates changed since the last call, and update the stored fingerprint.
    /// Returns `true` if candidates are the same as before (stagnant loop detected).
    pub fn check_candidates_stagnant(&mut self) -> bool {
        let fp = self.candidate_fingerprint();
        let stagnant = self.prev_candidate_fingerprint == Some(fp);
        self.prev_candidate_fingerprint = Some(fp);
        stagnant
    }

    /// Check if token limit is reached.
    pub fn is_token_limit_reached(&self) -> bool {
        self.token_count >= self.options.max_tokens
    }

    /// Calculate token utilization percentage.
    pub fn token_utilization(&self) -> f32 {
        if self.options.max_tokens == 0 {
            0.0
        } else {
            (self.token_count as f32 / self.options.max_tokens as f32).min(1.0)
        }
    }

    /// Append a reasoning step to the chain.
    pub fn push_reasoning_step(&mut self, step: ReasoningStep) {
        self.reasoning_chain.push(step);
    }

    /// Convenience: push a simple reasoning step with no node association.
    pub fn record_reasoning(
        &mut self,
        stage: StageName,
        reasoning: impl Into<String>,
        decision: NavigationDecision,
    ) {
        self.push_reasoning_step(ReasoningStep {
            stage,
            node_id: None,
            title: None,
            score: 0.0,
            decision,
            depth: 0,
            reasoning: reasoning.into(),
            candidates: Vec::new(),
            strategy_used: None,
            llm_call: None,
            references_followed: Vec::new(),
        });
    }

    /// Finalize the context into a response.
    pub fn finalize(self) -> RetrieveResponse {
        self.result.unwrap_or_else(|| RetrieveResponse {
            results: Vec::new(),
            content: self.accumulated_content,
            confidence: 0.0,
            is_sufficient: self.sufficiency == SufficiencyLevel::Sufficient,
            strategy_used: self
                .selected_strategy
                .map(|s| format!("{:?}", s))
                .unwrap_or_else(|| "unknown".to_string()),
            complexity: self.complexity.unwrap_or_default(),
            reasoning_chain: self.reasoning_chain,
            tokens_used: self.token_count,
        })
    }
}
