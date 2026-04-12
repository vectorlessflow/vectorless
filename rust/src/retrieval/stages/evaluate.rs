// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Evaluate Stage - Sufficiency checking.
//!
//! This stage evaluates whether the collected content is sufficient
//! to answer the query, and can trigger additional search iterations.

use async_trait::async_trait;
// Arc is used for async sharing
use tracing::{info, warn};

use crate::llm::LlmClient;
use crate::retrieval::content::{ContentAggregator, ContentAggregatorConfig};
use crate::retrieval::pipeline::{FailurePolicy, PipelineContext, RetrievalStage, StageOutcome};
use crate::retrieval::sufficiency::{LlmJudge, SufficiencyChecker, ThresholdChecker};
use crate::retrieval::types::{
    NavigationDecision, RetrievalResult, RetrieveResponse, StageName, SufficiencyLevel,
};
use crate::utils::estimate_tokens;

/// Evaluate Stage - evaluates retrieval sufficiency.
///
/// This stage:
/// 1. Aggregates content from candidates
/// 2. Checks if content is sufficient to answer the query
/// 3. Can trigger additional search iterations if needed
///
/// # Content Aggregation
///
/// By default, uses simple content collection. For precision-focused
/// aggregation with token budget control, use `with_content_aggregator()`.
///
/// # Example
///
/// ```rust,ignore
/// let stage = EvaluateStage::new()
///     .with_llm_judge(llm_client)
///     .with_max_iterations(3)
///     .with_content_aggregator(ContentAggregatorConfig::default());
/// ```
pub struct EvaluateStage {
    threshold_checker: ThresholdChecker,
    llm_judge: Option<LlmJudge>,
    max_iterations: usize,
    use_llm_judge: bool,
    /// Optional content aggregator for precision-focused aggregation.
    content_aggregator: Option<ContentAggregator>,
}

impl Default for EvaluateStage {
    fn default() -> Self {
        Self::new()
    }
}

impl EvaluateStage {
    /// Create a new evaluate stage.
    pub fn new() -> Self {
        Self {
            threshold_checker: ThresholdChecker::new(),
            llm_judge: None,
            max_iterations: 3,
            use_llm_judge: false,
            content_aggregator: None,
        }
    }

    /// Add LLM judge for more accurate sufficiency checking.
    pub fn with_llm_judge(mut self, client: LlmClient) -> Self {
        self.llm_judge = Some(LlmJudge::new(Box::new(client)));
        self.use_llm_judge = true;
        self
    }

    /// Set maximum search iterations.
    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    /// Add content aggregator for precision-focused aggregation.
    ///
    /// When enabled, content aggregation uses:
    /// - Relevance scoring (keyword + BM25)
    /// - Token budget allocation
    /// - Hierarchical content selection
    pub fn with_content_aggregator(mut self, config: ContentAggregatorConfig) -> Self {
        self.content_aggregator = Some(ContentAggregator::new(config));
        self
    }

    /// Enable content aggregator with default configuration.
    pub fn with_default_content_aggregator(mut self) -> Self {
        self.content_aggregator = Some(ContentAggregator::with_defaults());
        self
    }

    /// Aggregate content from candidates.
    ///
    /// When content aggregator is enabled:
    /// - Uses relevance scoring for content selection
    /// - Respects token budget
    /// - Prioritizes high-relevance content
    ///
    /// Otherwise falls back to simple collection:
    /// - Collects node's own content + descendant leaf content
    fn aggregate_content(&self, ctx: &PipelineContext) -> (String, usize) {
        // Use ContentAggregator if configured
        if let Some(ref aggregator) = self.content_aggregator {
            use crate::retrieval::content::CandidateNode;

            let candidates: Vec<CandidateNode> = ctx
                .candidates
                .iter()
                .map(|c| CandidateNode::new(c.node_id, c.score, c.depth))
                .collect();

            let result = aggregator.aggregate(&candidates, &ctx.tree, &ctx.query);
            info!(
                "ContentAggregator: {} nodes, {} tokens, avg score {:.2}",
                result.nodes_included, result.tokens_used, result.avg_score
            );
            return (result.content, result.tokens_used);
        }

        // Fallback: simple content collection
        self.aggregate_content_simple(ctx)
    }

    /// Simple content aggregation (legacy behavior).
    fn aggregate_content_simple(&self, ctx: &PipelineContext) -> (String, usize) {
        let mut content_parts = Vec::new();
        let mut total_tokens = 0;

        for candidate in &ctx.candidates {
            if let Some(node) = ctx.tree.get(candidate.node_id) {
                // Add title
                content_parts.push(format!("## {}\n", node.title));

                // Always collect all content: own content + descendant leaf content
                let mut has_content = false;

                // Add node's own content if available
                if !node.content.is_empty() {
                    content_parts.push(format!("{}\n\n", node.content));
                    has_content = true;
                }

                // Also collect content from leaf descendants (for intermediate nodes)
                let leaf_content = self.collect_leaf_content(&ctx.tree, candidate.node_id);
                if !leaf_content.is_empty() {
                    content_parts.push(format!("{}\n\n", leaf_content));
                    has_content = true;
                }

                // Fall back to summary only if no content available
                if !has_content && !node.summary.is_empty() {
                    content_parts.push(format!("{}\n\n", node.summary));
                }

                // Estimate tokens
                total_tokens += estimate_tokens(&content_parts.last().unwrap_or(&String::new()));
            }
        }

        (content_parts.join(""), total_tokens)
    }

    /// Collect content from leaf descendants of a node (excluding the node itself).
    ///
    /// Uses BFS (FIFO) traversal to preserve document order — the first
    /// section in the document appears first in the output.
    fn collect_leaf_content(
        &self,
        tree: &crate::document::DocumentTree,
        node_id: crate::document::NodeId,
    ) -> String {
        use std::collections::VecDeque;

        let mut content_parts = Vec::new();

        // Start with children, not the node itself
        let children = tree.children(node_id);
        if children.is_empty() {
            // Node is already a leaf, no descendants to collect
            return String::new();
        }

        let mut queue: VecDeque<crate::document::NodeId> = children.into_iter().collect();

        while let Some(current_id) = queue.pop_front() {
            let current_children = tree.children(current_id);

            if current_children.is_empty() {
                // Leaf node - collect its content
                if let Some(node) = tree.get(current_id) {
                    if !node.content.is_empty() {
                        content_parts.push(format!("### {}\n{}", node.title, node.content));
                    }
                }
            } else {
                // Non-leaf node - add children to queue (FIFO preserves order)
                queue.extend(current_children);
            }
        }

        content_parts.join("\n\n")
    }

    /// Check sufficiency level.
    fn check_sufficiency(&self, ctx: &PipelineContext) -> SufficiencyLevel {
        if !ctx.options.sufficiency_check {
            return SufficiencyLevel::Sufficient;
        }

        // Use LLM evaluate if available and enabled
        if self.use_llm_judge {
            if let Some(ref evaluate) = self.llm_judge {
                return evaluate.check(&ctx.query, &ctx.accumulated_content, ctx.token_count);
            }
        }

        // Fall back to threshold checker
        self.threshold_checker
            .check(&ctx.query, &ctx.accumulated_content, ctx.token_count)
    }

    /// Build the final response.
    fn build_response(&self, ctx: &PipelineContext) -> RetrieveResponse {
        let mut results = Vec::new();

        for candidate in &ctx.candidates {
            if let Some(node) = ctx.tree.get(candidate.node_id) {
                // Build content: node's own content + all descendant leaf content
                let content = if ctx.options.include_content {
                    let mut content_parts = Vec::new();

                    // Add node's own content
                    if !node.content.is_empty() {
                        content_parts.push(node.content.clone());
                    }

                    // Add content from leaf descendants
                    let leaf_content = self.collect_leaf_content(&ctx.tree, candidate.node_id);
                    if !leaf_content.is_empty() {
                        content_parts.push(leaf_content);
                    }

                    if content_parts.is_empty() {
                        None
                    } else {
                        Some(content_parts.join("\n\n"))
                    }
                } else {
                    None
                };

                results.push(RetrievalResult {
                    node_id: Some(format!("{:?}", candidate.node_id)),
                    title: node.title.clone(),
                    content,
                    summary: if ctx.options.include_summaries {
                        Some(node.summary.clone())
                    } else {
                        None
                    },
                    score: candidate.score,
                    depth: candidate.depth,
                    page_range: node.start_page.zip(node.end_page),
                });
            }
        }

        RetrieveResponse {
            results,
            content: ctx.accumulated_content.clone(),
            confidence: self.calculate_confidence(ctx),
            is_sufficient: ctx.sufficiency == SufficiencyLevel::Sufficient,
            strategy_used: ctx
                .selected_strategy
                .map(|s| format!("{:?}", s))
                .unwrap_or_else(|| "unknown".to_string()),
            complexity: ctx.complexity.unwrap_or_default(),
            reasoning_chain: ctx.reasoning_chain.clone(),
            tokens_used: ctx.token_count,
        }
    }

    /// Calculate overall confidence score.
    fn calculate_confidence(&self, ctx: &PipelineContext) -> f32 {
        if ctx.candidates.is_empty() {
            return 0.0;
        }

        // Weight by score and sufficiency
        let avg_score: f32 =
            ctx.candidates.iter().map(|c| c.score).sum::<f32>() / ctx.candidates.len() as f32;

        let sufficiency_factor = match ctx.sufficiency {
            SufficiencyLevel::Sufficient => 1.0,
            SufficiencyLevel::PartialSufficient => 0.7,
            SufficiencyLevel::Insufficient => 0.4,
        };

        avg_score * sufficiency_factor
    }
}

#[async_trait]
impl RetrievalStage for EvaluateStage {
    fn name(&self) -> &'static str {
        "evaluate"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["search"]
    }

    fn priority(&self) -> i32 {
        40 // Fourth stage
    }

    fn failure_policy(&self) -> FailurePolicy {
        FailurePolicy::skip() // Can skip if evaluate fails
    }

    fn can_backtrack(&self) -> bool {
        true // Can trigger backtracking to search
    }

    async fn execute(&self, ctx: &mut PipelineContext) -> crate::error::Result<StageOutcome> {
        let start = std::time::Instant::now();

        info!(
            "Judging sufficiency: {} candidates, iteration {}",
            ctx.candidates.len(),
            ctx.search_iterations
        );

        // 1. Aggregate content from candidates
        let (content, tokens) = self.aggregate_content(ctx);
        ctx.accumulated_content = content;
        ctx.token_count = tokens;

        info!("Aggregated {} tokens", tokens);

        // 2. Report token consumption to budget controller
        ctx.budget_controller.record_tokens(tokens);

        // 3. Check sufficiency
        ctx.sufficiency = self.check_sufficiency(ctx);
        info!("Sufficiency level: {:?}", ctx.sufficiency);

        // 3.5 Detect stagnant candidates (same results as previous iteration)
        // If candidates haven't changed, further backtracking won't help.
        let stagnant = ctx.check_candidates_stagnant();
        if stagnant {
            info!(
                "Candidates unchanged after backtrack, completing with {} candidates",
                ctx.candidates.len()
            );
            ctx.result = Some(self.build_response(ctx));
            ctx.record_reasoning(
                StageName::Evaluate,
                format!(
                    "Candidates stagnant (unchanged), forced completion with {} candidates",
                    ctx.candidates.len()
                ),
                NavigationDecision::Skip,
            );
            return Ok(StageOutcome::complete());
        }

        // Update metrics
        ctx.metrics.evaluate_time_ms += start.elapsed().as_millis() as u64;
        ctx.metrics.tokens_used = tokens;

        // 4. Check budget status for adaptive decision
        let budget_status = ctx.budget_controller.status();
        let confidence = self.calculate_confidence(ctx);

        // If budget is exhausted, force completion regardless of sufficiency
        if budget_status.should_stop() && ctx.search_iterations >= 1 {
            info!(
                "Budget exhausted ({}/{}), completing with current results",
                ctx.budget_controller.consumed(),
                ctx.budget_controller.total_budget(),
            );
            ctx.result = Some(self.build_response(ctx));
            ctx.record_reasoning(
                StageName::Evaluate,
                format!(
                    "Budget exhausted ({}/{}), forced completion; confidence={:.3}",
                    ctx.budget_controller.consumed(),
                    ctx.budget_controller.total_budget(),
                    confidence,
                ),
                NavigationDecision::Skip,
            );
            return Ok(StageOutcome::complete());
        }

        // 2.5 Record successful navigation paths to L2 cache
        if confidence > 0.5 {
            let doc_key = format!("{:?}", ctx.tree.root());
            for candidate in ctx.candidates.iter().take(3) {
                if let Some(node) = ctx.tree.get(candidate.node_id) {
                    let path = format!("{}", node.depth);
                    // Use the node title as path identifier for L2
                    ctx.reasoning_cache
                        .l2_record(&doc_key, &node.title, candidate.score);
                }
            }
        }

        // 3. Decide next action based on sufficiency
        let outcome = match ctx.sufficiency {
            SufficiencyLevel::Sufficient => {
                info!("Content is sufficient, completing retrieval");
                ctx.result = Some(self.build_response(ctx));
                StageOutcome::complete()
            }
            SufficiencyLevel::PartialSufficient => {
                // Can return current results or continue
                if ctx.search_iterations >= self.max_iterations {
                    info!(
                        "Partial sufficient but max iterations reached, completing with {} candidates",
                        ctx.candidates.len()
                    );
                    ctx.result = Some(self.build_response(ctx));
                    StageOutcome::complete()
                } else {
                    // Continue searching with small beam increase
                    info!("Partial sufficient, requesting one more search iteration");
                    StageOutcome::need_more(1, false)
                }
            }
            SufficiencyLevel::Insufficient => {
                if ctx.search_iterations >= self.max_iterations {
                    warn!(
                        "Insufficient but max iterations reached, returning {} candidates",
                        ctx.candidates.len()
                    );
                    ctx.result = Some(self.build_response(ctx));
                    StageOutcome::complete()
                } else {
                    // Need more data - increase beam and go deeper
                    info!("Insufficient content, requesting more search with larger beam");
                    StageOutcome::need_more(2, true)
                }
            }
        };

        // Update LLM call count if we used LLM evaluate
        if self.use_llm_judge && self.llm_judge.is_some() {
            ctx.metrics.llm_calls += 1;
        }

        // Record evaluation reasoning with budget status
        let sufficiency_str = format!("{:?}", ctx.sufficiency);
        let decision = match ctx.sufficiency {
            SufficiencyLevel::Sufficient => NavigationDecision::ThisIsTheAnswer,
            SufficiencyLevel::PartialSufficient => NavigationDecision::ExploreMore,
            SufficiencyLevel::Insufficient => NavigationDecision::ExploreMore,
        };
        ctx.record_reasoning(
            StageName::Evaluate,
            format!(
                "Sufficiency={}, confidence={:.3}, tokens={}, candidates={}, iteration={}, budget={:?} ({}/{})",
                sufficiency_str,
                self.calculate_confidence(ctx),
                ctx.token_count,
                ctx.candidates.len(),
                ctx.search_iterations,
                budget_status,
                ctx.budget_controller.consumed(),
                ctx.budget_controller.total_budget(),
            ),
            decision,
        );

        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_stage_creation() {
        let stage = EvaluateStage::new();
        assert!(stage.llm_judge.is_none());
        assert!(!stage.use_llm_judge);
    }

    #[test]
    fn test_evaluate_stage_dependencies() {
        let stage = EvaluateStage::new();
        assert_eq!(stage.depends_on(), vec!["search"]);
    }

    #[test]
    fn test_evaluate_can_backtrack() {
        let stage = EvaluateStage::new();
        assert!(stage.can_backtrack());
    }
}
