// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Search Stage - Execute tree search with Pilot integration.
//!
//! This stage executes the selected search algorithm using
//! the selected retrieval strategy. When a Pilot is provided,
//! it can provide semantic guidance at key decision points.
//!
//! # LLM-First Search
//!
//! When an LLM client is provided, the stage will first attempt to
//! directly locate the top-3 most relevant nodes using the TOC,
//! falling back to tree traversal algorithms (Beam/Greedy) only if
//! LLM fails or returns insufficient results.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

use crate::document::{DocumentTree, TocView};
use crate::llm::LlmClient;
use crate::retrieval::RetrievalContext; // Legacy context
use crate::retrieval::pilot::Pilot;
use crate::retrieval::pipeline::{
    CandidateNode, FailurePolicy, PipelineContext, RetrievalStage, SearchAlgorithm, StageOutcome,
};
use crate::retrieval::search::{
    BeamSearch, GreedySearch, SearchConfig as SearchAlgConfig, SearchTree,
};
use crate::retrieval::strategy::{
    HybridConfig, HybridStrategy, KeywordStrategy, LlmStrategy, RetrievalStrategy,
};
use crate::retrieval::types::StrategyPreference;

/// Search Stage - executes tree search with optional Pilot guidance.
///
/// This stage:
/// 1. Instantiates the selected search algorithm
/// 2. Creates the appropriate strategy
/// 3. Executes search with optional Pilot intervention
/// 4. Collects candidates
///
/// # Pilot Integration
///
/// When a Pilot is provided via [`with_pilot`], the search algorithm
/// can consult it at key decision points for semantic guidance.
/// Without a Pilot, the search uses pure algorithm scoring.
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::pilot::{LlmPilot, PilotConfig};
///
/// let pilot = LlmPilot::new(llm_client, PilotConfig::default());
/// let stage = SearchStage::new()
///     .with_pilot(Arc::new(pilot))
///     .with_llm_strategy(llm_strategy);
/// ```
pub struct SearchStage {
    keyword_strategy: KeywordStrategy,
    llm_strategy: Option<Arc<LlmStrategy>>,
    semantic_strategy: Option<Arc<dyn RetrievalStrategy>>,
    hybrid_strategy: Option<Arc<dyn RetrievalStrategy>>,
    /// Pilot for navigation guidance (optional).
    pilot: Option<Arc<dyn Pilot>>,
    /// LLM client for direct TOC-based search (optional).
    llm_client: Option<LlmClient>,
}

impl Default for SearchStage {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchStage {
    /// Create a new search stage without Pilot.
    pub fn new() -> Self {
        Self {
            keyword_strategy: KeywordStrategy::new(),
            llm_strategy: None,
            semantic_strategy: None,
            hybrid_strategy: None,
            pilot: None,
            llm_client: None,
        }
    }

    /// Add LLM client for direct TOC-based search.
    ///
    /// When provided, the stage will first attempt to locate relevant
    /// nodes directly using the TOC, falling back to tree traversal
    /// algorithms only if LLM fails or returns insufficient results.
    pub fn with_llm_client(mut self, client: Option<LlmClient>) -> Self {
        self.llm_client = client;
        self
    }

    /// Add Pilot for semantic navigation guidance.
    ///
    /// When provided, the search algorithm will consult the Pilot
    /// at key decision points to get semantic guidance on which
    /// branches are most relevant to the query.
    pub fn with_pilot(mut self, pilot: Arc<dyn Pilot>) -> Self {
        self.pilot = Some(pilot);
        self
    }

    /// Add LLM strategy for complex queries.
    pub fn with_llm_strategy(mut self, strategy: LlmStrategy) -> Self {
        self.llm_strategy = Some(Arc::new(strategy));
        self
    }

    /// Add semantic strategy for embedding-based search.
    pub fn with_semantic_strategy(mut self, strategy: Arc<dyn RetrievalStrategy>) -> Self {
        self.semantic_strategy = Some(strategy);
        self
    }

    /// Add hybrid strategy (BM25 + LLM refinement).
    ///
    /// If no LLM strategy is set, creates one from the provided LLM strategy.
    pub fn with_hybrid_strategy(mut self, strategy: Arc<dyn RetrievalStrategy>) -> Self {
        self.hybrid_strategy = Some(strategy);
        self
    }

    /// Configure hybrid strategy with custom config using the LLM strategy.
    pub fn with_hybrid_config(mut self, config: HybridConfig) -> Self {
        if let Some(ref llm) = self.llm_strategy {
            // Clone the LlmStrategy and box it
            let llm_boxed: Box<dyn RetrievalStrategy> = Box::new((**llm).clone());
            self.hybrid_strategy = Some(Arc::new(
                HybridStrategy::new(llm_boxed).with_config(config)
            ));
        }
        self
    }

    /// Check if Pilot is available and active.
    pub fn has_pilot(&self) -> bool {
        self.pilot.as_ref().map(|p| p.is_active()).unwrap_or(false)
    }

    /// Get the strategy to use based on context.
    fn get_strategy(&self, ctx: &PipelineContext) -> Arc<dyn RetrievalStrategy> {
        let preference = ctx.selected_strategy.unwrap_or(StrategyPreference::Auto);

        match preference {
            StrategyPreference::ForceKeyword => {
                info!("Using Keyword strategy");
                Arc::new(self.keyword_strategy.clone())
            }
            StrategyPreference::ForceSemantic => {
                if let Some(ref strategy) = self.semantic_strategy {
                    info!("Using Semantic strategy");
                    strategy.clone()
                } else {
                    warn!("Semantic strategy requested but not available, falling back to Keyword");
                    Arc::new(self.keyword_strategy.clone())
                }
            }
            StrategyPreference::ForceLlm => {
                if let Some(ref strategy) = self.llm_strategy {
                    info!("Using LLM strategy");
                    strategy.clone()
                } else {
                    warn!("LLM strategy requested but not available, falling back to Keyword");
                    Arc::new(self.keyword_strategy.clone())
                }
            }
            StrategyPreference::ForceHybrid => {
                if let Some(ref strategy) = self.hybrid_strategy {
                    info!("Using Hybrid strategy");
                    strategy.clone()
                } else if let Some(ref llm) = self.llm_strategy {
                    info!("Using Hybrid strategy (auto-created from LLM)");
                    let llm_boxed: Box<dyn RetrievalStrategy> = Box::new((**llm).clone());
                    Arc::new(HybridStrategy::new(llm_boxed))
                } else {
                    warn!("Hybrid strategy requested but no LLM available, falling back to Keyword");
                    Arc::new(self.keyword_strategy.clone())
                }
            }
            StrategyPreference::ForceCrossDocument | StrategyPreference::ForcePageRange => {
                // These require special setup, fall back to hybrid or keyword
                if let Some(ref strategy) = self.hybrid_strategy {
                    info!("Using Hybrid strategy as fallback for {:?})", preference);
                    strategy.clone()
                } else {
                    warn!("{:?} requires special configuration, falling back to Keyword", preference);
                    Arc::new(self.keyword_strategy.clone())
                }
            }
            StrategyPreference::Auto => {
                // Default to keyword, let plan stage decide
                Arc::new(self.keyword_strategy.clone())
            }
        }
    }

    /// Extract candidates from search paths.
    fn extract_candidates(
        &self,
        paths: &[crate::retrieval::types::SearchPath],
        tree: &DocumentTree,
    ) -> Vec<CandidateNode> {
        let mut candidates = Vec::new();

        for path in paths {
            if let Some(leaf_id) = path.leaf {
                // Get node info
                if let Some(node) = tree.get(leaf_id) {
                    let depth = node.depth;
                    let is_leaf = tree.is_leaf(leaf_id);

                    candidates.push(CandidateNode::new(leaf_id, path.score, depth, is_leaf));
                }
            }
        }

        // Sort by score descending
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates
    }

    /// Build a flat TOC list for LLM consumption.
    ///
    /// Returns a formatted string with numbered entries:
    /// ```
    /// [1] Title: "Overview"
    ///     Summary: "This section covers..."
    /// [2] Title: "Architecture"
    ///     Summary: "The system architecture..."
    /// ```
    fn build_toc_for_llm(&self, tree: &DocumentTree) -> (String, Vec<crate::document::NodeId>) {
        let toc_view = TocView::new();
        let mut entries = Vec::new();
        let mut node_ids = Vec::new();

        fn collect_entries(
            tree: &DocumentTree,
            node_id: crate::document::NodeId,
            entries: &mut Vec<(usize, String, String)>,
            node_ids: &mut Vec<crate::document::NodeId>,
            index: &mut usize,
        ) {
            if let Some(node) = tree.get(node_id) {
                let title = node.title.clone();
                let summary = if node.summary.is_empty() {
                    "(no summary)".to_string()
                } else {
                    node.summary.clone()
                };
                entries.push((*index, title, summary));
                node_ids.push(node_id);
                *index += 1;

                for child_id in tree.children(node_id) {
                    collect_entries(tree, child_id, entries, node_ids, index);
                }
            }
        }

        collect_entries(tree, tree.root(), &mut entries, &mut node_ids, &mut 0);

        let toc_str = entries
            .iter()
            .map(|(idx, title, summary)| {
                format!("[{}] Title: \"{}\"\n    Summary: \"{}\"", idx + 1, title, summary)
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        (toc_str, node_ids)
    }

    /// Locate top candidates directly via LLM using TOC.
    ///
    /// This method bypasses tree traversal by asking the LLM to
    /// directly identify the most relevant nodes from the TOC.
    async fn locate_via_llm(
        &self,
        query: &str,
        tree: &DocumentTree,
    ) -> Option<Vec<CandidateNode>> {
        let llm_client = self.llm_client.as_ref()?;
        let (toc_str, node_ids) = self.build_toc_for_llm(tree);

        if node_ids.is_empty() {
            warn!("No nodes in tree for LLM search");
            return None;
        }

        let system_prompt = r#"You are a document navigation assistant. Your task is to locate the most relevant sections in a document hierarchy for a user's query.

CRITICAL INSTRUCTIONS:
1. Analyze the user query carefully to understand the intent
2. Examine the provided Table of Contents (TOC) with numbered entries
3. Select the TOP 3 most relevant entries that would contain the answer
4. You MUST respond with ONLY valid JSON. No markdown code blocks. No explanations outside JSON.

Your response must have this EXACT structure:
{
  "reasoning": "Brief analysis of the query and why you selected these entries",
  "candidates": [
    {"node_id": 1, "relevance_score": 0.95, "reason": "Why this entry matches the query"},
    {"node_id": 2, "relevance_score": 0.80, "reason": "Why this entry is also relevant"},
    {"node_id": 3, "relevance_score": 0.65, "reason": "Why this entry might be relevant"}
  ]
}

Rules:
- node_id: MUST be a number from the provided TOC (the number in [N] brackets)
- relevance_score: Number between 0.0 and 1.0 (higher = more relevant)
- reason: Brief explanation for each selection
- candidates: Must have exactly 3 items, ordered by relevance (highest first)"#;

        let user_prompt = format!(
            "USER QUERY: {}\n\nDOCUMENT TOC ({} entries):\n{}\n\nBased on the query and TOC above, select the TOP 3 most relevant entries.\n\nRespond with ONLY the JSON object:",
            query,
            node_ids.len(),
            toc_str
        );

        info!("Attempting LLM-based search for query: '{}'", query);

        match llm_client.complete(system_prompt, &user_prompt).await {
            Ok(response) => {
                // Parse JSON response
                match serde_json::from_str::<LlmLocateResponse>(&response) {
                    Ok(llm_response) => {
                        let mut candidates = Vec::new();

                        for candidate in llm_response.candidates {
                            // node_id is 1-indexed from LLM, convert to 0-indexed
                            let idx = candidate.node_id.saturating_sub(1);
                            if idx < node_ids.len() {
                                let node_id = node_ids[idx];
                                if let Some(node) = tree.get(node_id) {
                                    candidates.push(CandidateNode::new(
                                        node_id,
                                        candidate.relevance_score,
                                        node.depth,
                                        tree.is_leaf(node_id),
                                    ));
                                    info!(
                                        "LLM selected: [{}] '{}' (score: {:.2})",
                                        candidate.node_id, node.title, candidate.relevance_score
                                    );
                                }
                            }
                        }

                        if candidates.is_empty() {
                            warn!("LLM returned no valid candidates");
                            return None;
                        }

                        println!("LLM search found {} candidates", candidates.len());
                        println!("LLM candidates content: {:?}", candidates);
                        Some(candidates)
                    }
                    Err(e) => {
                        warn!("Failed to parse LLM response as JSON: {}", e);
                        warn!("Raw response: {}", response);
                        None
                    }
                }
            }
            Err(e) => {
                warn!("LLM call failed: {}", e);
                None
            }
        }
    }
}

/// LLM response for locate query.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmLocateResponse {
    reasoning: String,
    candidates: Vec<LlmLocateCandidate>,
}

/// A candidate from LLM locate response.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmLocateCandidate {
    node_id: usize,
    relevance_score: f32,
    reason: String,
}

#[async_trait]
impl RetrievalStage for SearchStage {
    fn name(&self) -> &str {
        "search"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["plan"]
    }

    fn priority(&self) -> i32 {
        30 // Third stage
    }

    fn failure_policy(&self) -> FailurePolicy {
        FailurePolicy::retry() // Retry on transient failures
    }

    fn can_backtrack(&self) -> bool {
        true // Can receive backtracks from judge
    }

    async fn execute(&self, ctx: &mut PipelineContext) -> crate::error::Result<StageOutcome> {
        let start = std::time::Instant::now();

        // Get strategy and algorithm
        let _strategy = self.get_strategy(ctx);
        let algorithm = ctx.selected_algorithm.unwrap_or(SearchAlgorithm::Beam);
        let config = ctx.search_config.clone().unwrap_or_default();

        // Reset Pilot state for new query
        if let Some(ref pilot) = self.pilot {
            pilot.reset();
            println!("[DEBUG] SearchStage: Pilot is available, is_active={}", pilot.is_active());
        } else {
            println!("[DEBUG] SearchStage: No Pilot available");
        }

        info!(
            "Executing search: algorithm={:?}, beam_width={}, pilot={}",
            algorithm,
            config.beam_width,
            if self.has_pilot() {
                "enabled"
            } else {
                "disabled"
            }
        );

        // Increment search iteration
        ctx.increment_search_iteration();

        // === Try LLM-first search (direct TOC-based location) ===
        if self.llm_client.is_some() {
            info!("Attempting LLM-first search for query: '{}'", ctx.query);

            if let Some(candidates) = self.locate_via_llm(&ctx.query, &ctx.tree).await {
                if !candidates.is_empty() {
                    ctx.candidates = candidates;
                    ctx.metrics.search_time_ms += start.elapsed().as_millis() as u64;
                    ctx.metrics.nodes_visited += ctx.candidates.len();
                    ctx.metrics.llm_calls += 1;

                    info!(
                        "LLM-first search found {} candidates (skipped tree traversal)",
                        ctx.candidates.len()
                    );

                    return Ok(StageOutcome::cont());
                }
            }

            info!("LLM-first search returned no results, falling back to tree traversal");
        }

        // Build search config for search algorithms
        let search_config = SearchAlgConfig {
            top_k: config.beam_width * 2,
            beam_width: config.beam_width,
            max_iterations: config.max_iterations,
            min_score: config.min_score,
            leaf_only: false,
        };

        // Get Pilot reference (or None if not available)
        let pilot_ref: Option<&dyn Pilot> = self.pilot.as_deref();
        println!("[DEBUG] SearchStage: pilot_ref is {}", if pilot_ref.is_some() { "Some" } else { "None" });

        // === Check for decomposition ===
        if let Some(ref decomposition) = ctx.decomposition {
            if decomposition.was_decomposed && decomposition.is_multi_turn() {
                info!("Processing {} decomposed sub-queries", decomposition.sub_queries.len());

                let mut all_paths = Vec::new();
                let mut all_candidates = Vec::new();
                let mut total_pilot_interventions = 0u64;

                // Process each sub-query in execution order
                let order = decomposition.execution_order();
                for sub_idx in order {
                    let sub_query = &decomposition.sub_queries[sub_idx];
                    info!("Processing sub-query : {}", sub_query.text);

                    // Create legacy context for this sub-query
                    let legacy_ctx = RetrievalContext::new(
                        &sub_query.text,
                        ctx.options.max_tokens,
                        ctx.options.sufficiency_check,
                    );

                    println!("[DEBUG] SearchStage: Starting search for sub-query: algorithm={:?}, top_k={}, beam_width={}",
                        algorithm, search_config.top_k, search_config.beam_width);

                    // Execute search for this sub-query
                    let result = match algorithm {
                        SearchAlgorithm::Greedy => {
                            let search = GreedySearch::new();
                            search.search(&ctx.tree, &legacy_ctx, &search_config, pilot_ref).await
                        }
                        SearchAlgorithm::Beam => {
                            let search = BeamSearch::new();
                            search.search(&ctx.tree, &legacy_ctx, &search_config, pilot_ref).await
                        }
                        SearchAlgorithm::Mcts => {
                            let search = BeamSearch::new();
                            search.search(&ctx.tree, &legacy_ctx, &search_config, pilot_ref).await
                        }
                    };

                    all_candidates.extend(self.extract_candidates(&result.paths, &ctx.tree));
                    all_paths.extend(result.paths);
                    total_pilot_interventions += result.pilot_interventions as u64;

                    info!("Sub-query '{}' found {} paths", sub_query.text, all_paths.len());
                }

                // Merge results
                ctx.search_paths = all_paths;
                ctx.candidates = all_candidates;

                info!(
                    "Search complete: {} total candidates from {} sub-queries (pilot interventions: {})",
                    ctx.candidates.len(),
                    decomposition.sub_queries.len(),
                    total_pilot_interventions
                );
            } else {
                // Single query (not decomposed or single sub-query) - process as normal
                let legacy_ctx = RetrievalContext::new(
                    &ctx.query,
                    ctx.options.max_tokens,
                    ctx.options.sufficiency_check,
                );

                println!("[DEBUG] SearchStage: Starting search with algorithm={:?}, top_k={}, beam_width={}, max_iterations={}, min_score={:.2}",
                    algorithm, search_config.top_k, search_config.beam_width, search_config.max_iterations, search_config.min_score);

                let result = match algorithm {
                    SearchAlgorithm::Greedy => {
                        let search = GreedySearch::new();
                        search.search(&ctx.tree, &legacy_ctx, &search_config, pilot_ref).await
                    }
                    SearchAlgorithm::Beam => {
                        let search = BeamSearch::new();
                        search.search(&ctx.tree, &legacy_ctx, &search_config, pilot_ref).await
                    }
                    SearchAlgorithm::Mcts => {
                        let search = BeamSearch::new();
                        search.search(&ctx.tree, &legacy_ctx, &search_config, pilot_ref).await
                    }
                };

                ctx.search_paths = result.paths;
                ctx.candidates = self.extract_candidates(&ctx.search_paths, &ctx.tree);

                info!(
                    "Search found {} paths (pilot interventions: {})",
                    ctx.search_paths.len(),
                    result.pilot_interventions
                );
            }
        } else {
            // No decomposition available, process original query
            let legacy_ctx = RetrievalContext::new(
                &ctx.query,
                ctx.options.max_tokens,
                ctx.options.sufficiency_check,
            );

            println!("[DEBUG] SearchStage: Starting search with algorithm={:?}, top_k={}, beam_width={}, max_iterations={}, min_score={:.2}",
                algorithm, search_config.top_k, search_config.beam_width, search_config.max_iterations, search_config.min_score);

            let result = match algorithm {
                SearchAlgorithm::Greedy => {
                    let search = GreedySearch::new();
                    search.search(&ctx.tree, &legacy_ctx, &search_config, pilot_ref).await
                }
                SearchAlgorithm::Beam => {
                    let search = BeamSearch::new();
                    search.search(&ctx.tree, &legacy_ctx, &search_config, pilot_ref).await
                }
                SearchAlgorithm::Mcts => {
                    let search = BeamSearch::new();
                    search.search(&ctx.tree, &legacy_ctx, &search_config, pilot_ref).await
                }
            };

            ctx.search_paths = result.paths;
            ctx.candidates = self.extract_candidates(&ctx.search_paths, &ctx.tree);

            info!(
                "Search found {} paths (pilot interventions: {})",
                ctx.search_paths.len(),
                result.pilot_interventions
            );
        }

        // Debug output
        println!("[DEBUG] Search found {} total paths, {} candidates", ctx.search_paths.len(), ctx.candidates.len());
        for (i, path) in ctx.search_paths.iter().enumerate().take(5) {
            if let Some(leaf_id) = path.leaf {
                if let Some(node) = ctx.tree.get(leaf_id) {
                    println!("[DEBUG] Path {}: score={:.3}, title='{}', content_len={}",
                        i, path.score, node.title, node.content.len());
                }
            }
        }

        // Debug output
        println!("[DEBUG] Extracted {} candidates", ctx.candidates.len());
        for (i, c) in ctx.candidates.iter().enumerate().take(5) {
            if let Some(node) = ctx.tree.get(c.node_id) {
                println!("[DEBUG] Candidate {}: score={:.3}, title='{}'",
                    i, c.score, node.title);
            }
        }

        // Update metrics
        ctx.metrics.search_time_ms += start.elapsed().as_millis() as u64;
        ctx.metrics.nodes_visited += ctx.candidates.len();

        info!(
            "Search complete: {} candidates (iteration {})",
            ctx.candidates.len(),
            ctx.search_iterations
        );

        Ok(StageOutcome::cont())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::pilot::NoopPilot;

    #[test]
    fn test_search_stage_creation() {
        let stage = SearchStage::new();
        assert!(stage.llm_strategy.is_none());
        assert!(stage.semantic_strategy.is_none());
        assert!(!stage.has_pilot());
    }

    #[test]
    fn test_search_stage_dependencies() {
        let stage = SearchStage::new();
        assert_eq!(stage.depends_on(), vec!["plan"]);
    }

    #[test]
    fn test_search_stage_with_noop_pilot() {
        let pilot = Arc::new(NoopPilot::new());
        let stage = SearchStage::new().with_pilot(pilot);

        // NoopPilot is not active
        assert!(!stage.has_pilot());
    }
}
