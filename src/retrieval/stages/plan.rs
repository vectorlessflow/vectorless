// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Plan Stage - Strategy and algorithm selection.
//!
//! This stage selects:
//! - Retrieval strategy (Keyword/Semantic/LLM)
//! - Search algorithm (Greedy/Beam/MCTS)
//! - Search configuration

use async_trait::async_trait;
use std::sync::Arc;
use tracing::info;

use crate::domain::DocumentTree;
use crate::llm::LlmClient;
use crate::retrieval::pipeline::{
    FailurePolicy, PipelineContext, RetrievalStage, StageOutcome,
    SearchAlgorithm, SearchConfig,
};
use crate::retrieval::types::{QueryComplexity, StrategyPreference};

/// Plan Stage - plans the retrieval strategy.
///
/// This stage:
/// 1. Selects the appropriate retrieval strategy based on complexity
/// 2. Chooses the search algorithm
/// 3. Configures search parameters
///
/// # Example
///
/// ```rust,ignore
/// let stage = PlanStage::new()
///     .with_llm_client(llm_client);
/// ```
pub struct PlanStage {
    llm_client: Option<Arc<LlmClient>>,
}

impl Default for PlanStage {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanStage {
    /// Create a new plan stage.
    pub fn new() -> Self {
        Self { llm_client: None }
    }

    /// Set LLM client for complex planning.
    pub fn with_llm_client(mut self, client: LlmClient) -> Self {
        self.llm_client = Some(Arc::new(client));
        self
    }

    /// Select retrieval strategy based on complexity and preferences.
    fn select_strategy(&self, ctx: &PipelineContext) -> StrategyPreference {
        // Respect explicit strategy preference
        if ctx.options.strategy != StrategyPreference::Auto {
            info!("Using explicit strategy: {:?}", ctx.options.strategy);
            return ctx.options.strategy;
        }

        // Auto-select based on complexity
        let complexity = ctx.complexity.unwrap_or(QueryComplexity::Medium);

        let strategy = match complexity {
            QueryComplexity::Simple => {
                info!("Complexity is Simple, selecting Keyword strategy");
                StrategyPreference::ForceKeyword
            }
            QueryComplexity::Medium => {
                // Use semantic if available, otherwise keyword with LLM fallback
                if self.llm_client.is_some() {
                    info!("Complexity is Medium, selecting LLM strategy");
                    StrategyPreference::ForceLlm
                } else {
                    info!("Complexity is Medium, no LLM, selecting Keyword strategy");
                    StrategyPreference::ForceKeyword
                }
            }
            QueryComplexity::Complex => {
                if self.llm_client.is_some() {
                    info!("Complexity is Complex, selecting LLM strategy");
                    StrategyPreference::ForceLlm
                } else {
                    info!("Complexity is Complex, no LLM, selecting Keyword strategy");
                    StrategyPreference::ForceKeyword
                }
            }
        };

        strategy
    }

    /// Select search algorithm based on complexity and options.
    fn select_algorithm(&self, ctx: &PipelineContext) -> SearchAlgorithm {
        let complexity = ctx.complexity.unwrap_or(QueryComplexity::Medium);

        let algorithm = match complexity {
            QueryComplexity::Simple => {
                // Simple queries can use greedy search
                SearchAlgorithm::Greedy
            }
            QueryComplexity::Medium => {
                // Medium queries benefit from beam search
                SearchAlgorithm::Beam
            }
            QueryComplexity::Complex => {
                // Complex queries may benefit from MCTS
                // But for now, use beam search as MCTS is more expensive
                SearchAlgorithm::Beam
            }
        };

        info!("Selected search algorithm: {:?}", algorithm);
        algorithm
    }

    /// Build search configuration from options and complexity.
    fn build_search_config(&self, ctx: &PipelineContext) -> SearchConfig {
        let complexity = ctx.complexity.unwrap_or(QueryComplexity::Medium);

        let (beam_width, max_depth) = match complexity {
            QueryComplexity::Simple => (1, 5),  // Greedy-like
            QueryComplexity::Medium => (ctx.options.beam_width, 10),
            QueryComplexity::Complex => (ctx.options.beam_width + 2, 15),
        };

        SearchConfig {
            beam_width,
            max_depth,
            min_score: ctx.options.min_score,
            max_iterations: ctx.options.max_iterations,
        }
    }
}

#[async_trait]
impl RetrievalStage for PlanStage {
    fn name(&self) -> &str {
        "plan"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["analyze"]
    }

    fn priority(&self) -> i32 {
        20 // Second stage
    }

    fn failure_policy(&self) -> FailurePolicy {
        FailurePolicy::fail() // Must succeed
    }

    async fn execute(&self, ctx: &mut PipelineContext) -> crate::domain::Result<StageOutcome> {
        info!("Planning retrieval strategy");

        // 1. Select strategy
        ctx.selected_strategy = Some(self.select_strategy(ctx));

        // 2. Select algorithm
        ctx.selected_algorithm = Some(self.select_algorithm(ctx));

        // 3. Build search config
        ctx.search_config = Some(self.build_search_config(ctx));

        info!(
            "Plan complete: strategy={:?}, algorithm={:?}, beam_width={}",
            ctx.selected_strategy,
            ctx.selected_algorithm,
            ctx.search_config.as_ref().map(|c| c.beam_width).unwrap_or(0)
        );

        Ok(StageOutcome::cont())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_stage_creation() {
        let stage = PlanStage::new();
        assert!(stage.llm_client.is_none());
    }

    #[test]
    fn test_plan_stage_dependencies() {
        let stage = PlanStage::new();
        assert_eq!(stage.depends_on(), vec!["analyze"]);
    }
}
