// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Search Stage - Execute tree search.
//!
//! This stage executes the selected search algorithm using
//! the selected retrieval strategy.

use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, warn};

use crate::domain::DocumentTree;
// LlmClient is used via strategy
use crate::retrieval::pipeline::{
    FailurePolicy, PipelineContext, RetrievalStage, StageOutcome,
    CandidateNode, SearchAlgorithm,
};
use crate::retrieval::search::{BeamSearch, GreedySearch, SearchConfig as SearchAlgConfig, SearchTree};
use crate::retrieval::RetrievalContext; // Legacy context
use crate::retrieval::strategy::{KeywordStrategy, LlmStrategy, RetrievalStrategy};
use crate::retrieval::types::StrategyPreference;

/// Search Stage - executes tree search.
///
/// This stage:
/// 1. Instantiates the selected search algorithm
/// 2. Creates the appropriate strategy
/// 3. Executes search and collects candidates
///
/// # Example
///
/// ```rust,ignore
/// let stage = SearchStage::new()
///     .with_llm_strategy(llm_strategy);
/// ```
pub struct SearchStage {
    keyword_strategy: KeywordStrategy,
    llm_strategy: Option<Arc<LlmStrategy>>,
    semantic_strategy: Option<Arc<dyn RetrievalStrategy>>,
}

impl Default for SearchStage {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchStage {
    /// Create a new search stage.
    pub fn new() -> Self {
        Self {
            keyword_strategy: KeywordStrategy::new(),
            llm_strategy: None,
            semantic_strategy: None,
        }
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
                    let is_leaf = tree.children(leaf_id).is_empty();

                    candidates.push(CandidateNode::new(leaf_id, path.score, depth, is_leaf));
                }
            }
        }

        // Sort by score descending
        candidates.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates
    }
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

    async fn execute(&self, ctx: &mut PipelineContext) -> crate::domain::Result<StageOutcome> {
        let start = std::time::Instant::now();

        // Get strategy and algorithm
        let strategy = self.get_strategy(ctx);
        let algorithm = ctx.selected_algorithm.unwrap_or(SearchAlgorithm::Beam);
        let config = ctx.search_config.clone().unwrap_or_default();

        info!(
            "Executing search: algorithm={:?}, beam_width={}",
            algorithm, config.beam_width
        );

        // Increment search iteration
        ctx.increment_search_iteration();

        // Build search config for search algorithms
        let search_config = SearchAlgConfig {
            top_k: config.beam_width * 2,
            beam_width: config.beam_width,
            max_iterations: config.max_iterations,
            min_score: config.min_score,
            leaf_only: false,
        };

        // Create legacy context for search algorithms
        let legacy_ctx = RetrievalContext::new(&ctx.query, ctx.options.max_tokens, ctx.options.sufficiency_check);

        // Execute search based on algorithm
        let result = match algorithm {
            SearchAlgorithm::Greedy => {
                let search = GreedySearch::new();
                search.search(&ctx.tree, &legacy_ctx, &search_config).await
            }
            SearchAlgorithm::Beam => {
                let search = BeamSearch::new();
                search.search(&ctx.tree, &legacy_ctx, &search_config).await
            }
            SearchAlgorithm::Mcts => {
                // Use beam search as fallback for now
                let search = BeamSearch::new();
                search.search(&ctx.tree, &legacy_ctx, &search_config).await
            }
        };

        info!("Search found {} paths", result.paths.len());

        // Update context with results
        ctx.search_paths = result.paths.clone();
        ctx.candidates = self.extract_candidates(&result.paths, &ctx.tree);

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

    #[test]
    fn test_search_stage_creation() {
        let stage = SearchStage::new();
        assert!(stage.llm_strategy.is_none());
        assert!(stage.semantic_strategy.is_none());
    }

    #[test]
    fn test_search_stage_dependencies() {
        let stage = SearchStage::new();
        assert_eq!(stage.depends_on(), vec!["plan"]);
    }
}
