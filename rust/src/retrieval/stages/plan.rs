// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Plan Stage - Strategy and algorithm selection.
//!
//! This stage selects:
//! - Retrieval strategy (Keyword/Semantic/LLM)
//! - Search algorithm (PurePilot/Beam/MCTS)
//! - Search configuration

use async_trait::async_trait;
use std::sync::Arc;
use tracing::info;

// DocumentTree is accessed via context
use crate::llm::LlmClient;
use crate::retrieval::pipeline::{
    BudgetStatus, FailurePolicy, PipelineContext, RetrievalStage, SearchAlgorithm, SearchConfig,
    StageOutcome,
};
use crate::retrieval::types::{NavigationDecision, QueryComplexity, StageName, StrategyPreference};

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

    /// Select retrieval strategy based on complexity, preferences, and budget.
    fn select_strategy(&self, ctx: &PipelineContext) -> StrategyPreference {
        // Respect explicit strategy preference
        if ctx.options.strategy != StrategyPreference::Auto {
            info!("Using explicit strategy: {:?}", ctx.options.strategy);
            return ctx.options.strategy;
        }

        // Budget-aware strategy selection
        let budget_status = ctx.budget_controller.status();
        if budget_status.should_stop() {
            info!("Budget exhausted, forcing Keyword strategy");
            return StrategyPreference::ForceKeyword;
        }

        // Auto-select based on complexity
        let complexity = ctx.complexity.unwrap_or(QueryComplexity::Medium);

        let strategy = match complexity {
            QueryComplexity::Simple => {
                info!("Complexity is Simple, selecting Keyword strategy");
                StrategyPreference::ForceKeyword
            }
            QueryComplexity::Medium => {
                if budget_status == BudgetStatus::Constrained {
                    info!(
                        "Complexity is Medium but budget constrained, selecting Keyword strategy"
                    );
                    StrategyPreference::ForceKeyword
                } else if self.llm_client.is_some() {
                    info!("Complexity is Medium, selecting LLM strategy");
                    StrategyPreference::ForceLlm
                } else {
                    info!("Complexity is Medium, no LLM, selecting Keyword strategy");
                    StrategyPreference::ForceKeyword
                }
            }
            QueryComplexity::Complex => {
                if budget_status == BudgetStatus::Constrained {
                    info!(
                        "Complexity is Complex but budget constrained, selecting Hybrid strategy"
                    );
                    if self.llm_client.is_some() {
                        StrategyPreference::ForceHybrid
                    } else {
                        StrategyPreference::ForceKeyword
                    }
                } else if self.llm_client.is_some() {
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
                // Simple queries: PurePilot (beam=1, fast)
                SearchAlgorithm::PurePilot
            }
            QueryComplexity::Medium => {
                // Medium queries: Beam search
                SearchAlgorithm::Beam
            }
            QueryComplexity::Complex => {
                // Complex queries: MCTS for thorough exploration
                SearchAlgorithm::Mcts
            }
        };

        info!("Selected search algorithm: {:?}", algorithm);
        algorithm
    }

    /// Build search configuration from options and complexity.
    fn build_search_config(&self, ctx: &PipelineContext) -> SearchConfig {
        let complexity = ctx.complexity.unwrap_or(QueryComplexity::Medium);

        let (beam_width, max_depth) = match complexity {
            QueryComplexity::Simple => (1, 5), // PurePilot-like
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
    fn name(&self) -> &'static str {
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

    async fn execute(&self, ctx: &mut PipelineContext) -> crate::error::Result<StageOutcome> {
        info!("Planning retrieval strategy");

        // 1. Select strategy
        ctx.selected_strategy = Some(self.select_strategy(ctx));

        // 2. Select algorithm
        ctx.selected_algorithm = Some(self.select_algorithm(ctx));

        // 3. Build search config
        ctx.search_config = Some(self.build_search_config(ctx));

        // 4. Build fallback chain: primary algorithm first, then alternatives
        //    The chain determines which algorithms to try if the primary
        //    doesn't produce results above min_score.
        let primary = ctx.selected_algorithm.unwrap_or(SearchAlgorithm::Beam);
        let default_chain = vec![
            SearchAlgorithm::Beam,
            SearchAlgorithm::Mcts,
            SearchAlgorithm::PurePilot,
        ];
        // Remove primary from default chain, prepend it
        let mut chain = vec![primary];
        for algo in default_chain {
            if algo != primary {
                chain.push(algo);
            }
        }
        ctx.search_fallback_chain = chain;

        info!(
            "Plan complete: strategy={:?}, algorithm={:?}, beam_width={}",
            ctx.selected_strategy,
            ctx.selected_algorithm,
            ctx.search_config
                .as_ref()
                .map(|c| c.beam_width)
                .unwrap_or(0)
        );

        // Record reasoning
        let strategy_str = ctx
            .selected_strategy
            .map(|s| format!("{:?}", s))
            .unwrap_or_else(|| "auto".to_string());
        let algorithm_str = ctx
            .selected_algorithm
            .map(|a| a.name().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let beam_width = ctx
            .search_config
            .as_ref()
            .map(|c| c.beam_width)
            .unwrap_or(3);
        ctx.record_reasoning(
            StageName::Plan,
            format!(
                "Selected strategy={}, algorithm={}, beam_width={}; budget: {}/{} ({:.0}%)",
                strategy_str,
                algorithm_str,
                beam_width,
                ctx.budget_controller.consumed(),
                ctx.budget_controller.total_budget(),
                ctx.budget_controller.utilization() * 100.0
            ),
            NavigationDecision::ExploreMore,
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
