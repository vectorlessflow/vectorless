// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Hybrid retrieval strategy combining BM25 pre-filtering with LLM refinement.
//!
//! This two-stage approach minimizes LLM calls while maintaining high accuracy:
//! 1. **BM25 Filter**: Fast keyword scoring to identify candidate nodes
//! 2. **LLM Refinement**: Semantic understanding of top candidates only

use async_trait::async_trait;

use super::r#trait::{NodeEvaluation, RetrievalStrategy, StrategyCapabilities};
use crate::document::{DocumentTree, NodeId};
use crate::retrieval::search::{Bm25Engine, FieldDocument};
use crate::retrieval::types::{NavigationDecision, QueryComplexity};
use crate::retrieval::RetrievalContext;

/// Configuration for hybrid retrieval.
#[derive(Debug, Clone)]
pub struct HybridConfig {
    /// BM25 pre-filter: keep top N% of candidates.
    pub pre_filter_ratio: f32,
    /// BM25 pre-filter: minimum candidates to keep.
    pub min_candidates: usize,
    /// BM25 pre-filter: maximum candidates to pass to LLM.
    pub max_candidates: usize,
    /// Score threshold for automatic acceptance (skip LLM).
    pub auto_accept_threshold: f32,
    /// Score threshold for automatic rejection (skip LLM).
    pub auto_reject_threshold: f32,
    /// Weight for BM25 score in final scoring.
    pub bm25_weight: f32,
    /// Weight for LLM score in final scoring.
    pub llm_weight: f32,
    /// Whether to use BM25 for initial filtering.
    pub use_pre_filter: bool,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            pre_filter_ratio: 0.3,   // Keep top 30%
            min_candidates: 2,
            max_candidates: 5,
            auto_accept_threshold: 0.85,
            auto_reject_threshold: 0.15,
            bm25_weight: 0.4,
            llm_weight: 0.6,
            use_pre_filter: true,
        }
    }
}

impl HybridConfig {
    /// Create a new configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set pre-filter ratio.
    #[must_use]
    pub fn with_pre_filter_ratio(mut self, ratio: f32) -> Self {
        self.pre_filter_ratio = ratio.clamp(0.1, 1.0);
        self
    }

    /// Set candidate limits.
    #[must_use]
    pub fn with_candidate_limits(mut self, min: usize, max: usize) -> Self {
        self.min_candidates = min;
        self.max_candidates = max;
        self
    }

    /// Set score thresholds.
    #[must_use]
    pub fn with_thresholds(mut self, auto_accept: f32, auto_reject: f32) -> Self {
        self.auto_accept_threshold = auto_accept;
        self.auto_reject_threshold = auto_reject;
        self
    }

    /// Set scoring weights.
    #[must_use]
    pub fn with_weights(mut self, bm25: f32, llm: f32) -> Self {
        self.bm25_weight = bm25;
        self.llm_weight = llm;
        self
    }

    /// Disable pre-filtering (pass all to LLM).
    #[must_use]
    pub fn without_pre_filter(mut self) -> Self {
        self.use_pre_filter = false;
        self
    }

    /// High-quality mode (more LLM calls).
    #[must_use]
    pub fn high_quality() -> Self {
        Self {
            pre_filter_ratio: 0.5,
            min_candidates: 3,
            max_candidates: 8,
            auto_accept_threshold: 0.95,
            auto_reject_threshold: 0.1,
            bm25_weight: 0.3,
            llm_weight: 0.7,
            use_pre_filter: true,
        }
    }

    /// Low-cost mode (fewer LLM calls).
    #[must_use]
    pub fn low_cost() -> Self {
        Self {
            pre_filter_ratio: 0.2,
            min_candidates: 1,
            max_candidates: 3,
            auto_accept_threshold: 0.75,
            auto_reject_threshold: 0.25,
            bm25_weight: 0.5,
            llm_weight: 0.5,
            use_pre_filter: true,
        }
    }
}

/// Hybrid retrieval strategy combining BM25 and LLM.
///
/// This strategy uses a two-stage approach:
/// 1. **BM25 Filter**: Quickly score all nodes using keyword matching
/// 2. **LLM Refinement**: Apply semantic understanding to top candidates
///
/// This dramatically reduces LLM calls while maintaining accuracy.
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::strategy::{HybridStrategy, LlmStrategy};
///
/// let hybrid = HybridStrategy::new(
///     llm_strategy,
/// ).with_config(HybridConfig::high_quality());
/// ```
pub struct HybridStrategy {
    /// LLM strategy for refinement.
    llm_strategy: Box<dyn RetrievalStrategy>,
    /// Configuration.
    config: HybridConfig,
    /// BM25 engine for pre-filtering.
    bm25_engine: Option<Bm25Engine<usize>>,
}

impl HybridStrategy {
    /// Create a new hybrid strategy.
    pub fn new(llm_strategy: Box<dyn RetrievalStrategy>) -> Self {
        Self {
            llm_strategy,
            config: HybridConfig::default(),
            bm25_engine: None,
        }
    }

    /// Create with configuration.
    pub fn with_config(mut self, config: HybridConfig) -> Self {
        self.config = config;
        self
    }

    /// Set configuration for high-quality mode.
    pub fn with_high_quality(mut self) -> Self {
        self.config = HybridConfig::high_quality();
        self
    }

    /// Set configuration for low-cost mode.
    pub fn with_low_cost(mut self) -> Self {
        self.config = HybridConfig::low_cost();
        self
    }

    /// Build BM25 index from tree nodes.
    fn build_bm25_index(&mut self, tree: &DocumentTree, node_ids: &[NodeId]) {
        let documents: Vec<FieldDocument<usize>> = node_ids
            .iter()
            .enumerate()
            .map(|(idx, &node_id)| {
                if let Some(node) = tree.get(node_id) {
                    FieldDocument::new(
                        idx,
                        node.title.clone(),
                        node.summary.clone(),
                        node.content.clone(),
                    )
                } else {
                    FieldDocument::new(idx, String::new(), String::new(), String::new())
                }
            })
            .collect();

        if !documents.is_empty() {
            self.bm25_engine = Some(Bm25Engine::fit_to_corpus(&documents));
        }
    }

    /// Get BM25 scores for a query.
    fn bm25_scores(&self, query: &str, node_count: usize) -> Vec<(usize, f32)> {
        let engine = match &self.bm25_engine {
            Some(e) => e,
            None => return Vec::new(),
        };

        let results = engine.search_weighted(query, node_count);
        results
            .into_iter()
            .map(|(idx, score)| (idx, score))
            .collect()
    }

    /// Filter candidates using BM25 scores.
    fn filter_candidates(&self, bm25_scores: &[(usize, f32)], total_count: usize) -> Vec<usize> {
        if !self.config.use_pre_filter || total_count <= self.config.min_candidates {
            return (0..total_count).collect();
        }

        // Calculate how many candidates to keep
        let keep_count = ((total_count as f32 * self.config.pre_filter_ratio) as usize)
            .max(self.config.min_candidates)
            .min(self.config.max_candidates)
            .min(total_count);

        // Sort by score and take top candidates
        let mut sorted: Vec<_> = bm25_scores.to_vec();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        sorted
            .into_iter()
            .take(keep_count)
            .map(|(idx, _)| idx)
            .collect()
    }

    /// Combine BM25 and LLM scores.
    fn combine_scores(&self, bm25_score: f32, llm_score: f32) -> f32 {
        (bm25_score * self.config.bm25_weight + llm_score * self.config.llm_weight)
            / (self.config.bm25_weight + self.config.llm_weight)
    }
}

#[async_trait]
impl RetrievalStrategy for HybridStrategy {
    async fn evaluate_node(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        context: &RetrievalContext,
    ) -> NodeEvaluation {
        // Delegate to LLM strategy for single node
        self.llm_strategy.evaluate_node(tree, node_id, context).await
    }

    async fn evaluate_nodes(
        &self,
        tree: &DocumentTree,
        node_ids: &[NodeId],
        context: &RetrievalContext,
    ) -> Vec<NodeEvaluation> {
        if node_ids.is_empty() {
            return Vec::new();
        }

        // Build BM25 index if needed
        let bm25_scores = self.bm25_scores(&context.query, node_ids.len());

        // If no BM25 scores available, fall back to LLM only
        if bm25_scores.is_empty() {
            return self.llm_strategy.evaluate_nodes(tree, node_ids, context).await;
        }

        // Create a score map for quick lookup
        let score_map: std::collections::HashMap<usize, f32> = bm25_scores
            .iter()
            .map(|(idx, score)| (*idx, *score))
            .collect();

        // Normalize BM25 scores
        let max_bm25 = score_map.values().cloned().fold(0.0_f32, f32::max);
        let normalized_scores: std::collections::HashMap<usize, f32> = if max_bm25 > 0.0 {
            score_map
                .iter()
                .map(|(idx, score)| (*idx, *score / max_bm25))
                .collect()
        } else {
            score_map
        };

        // Check for auto-accept/reject candidates
        let mut results = vec![NodeEvaluation::default(); node_ids.len()];
        let mut needs_llm = Vec::new();

        for (idx, &node_id) in node_ids.iter().enumerate() {
            let bm25_score = normalized_scores.get(&idx).copied().unwrap_or(0.0);

            if bm25_score >= self.config.auto_accept_threshold {
                // High BM25 score: auto-accept without LLM
                results[idx] = NodeEvaluation {
                    score: bm25_score,
                    decision: if tree.is_leaf(node_id) {
                        NavigationDecision::ThisIsTheAnswer
                    } else {
                        NavigationDecision::ExploreMore
                    },
                    reasoning: Some(format!("Auto-accepted by BM25: {:.3}", bm25_score)),
                };
            } else if bm25_score <= self.config.auto_reject_threshold {
                // Low BM25 score: auto-reject without LLM
                results[idx] = NodeEvaluation {
                    score: bm25_score,
                    decision: NavigationDecision::Skip,
                    reasoning: Some(format!("Auto-rejected by BM25: {:.3}", bm25_score)),
                };
            } else {
                // Need LLM refinement
                needs_llm.push((idx, node_id, bm25_score));
            }
        }

        // Filter candidates for LLM
        let candidate_indices: std::collections::HashSet<usize> = self
            .filter_candidates(&bm25_scores, node_ids.len())
            .into_iter()
            .collect();

        // Only send to LLM if in candidates and not already processed
        let llm_nodes: Vec<NodeId> = needs_llm
            .iter()
            .filter(|(idx, _, _)| candidate_indices.contains(idx))
            .map(|(_, node_id, _)| *node_id)
            .collect();

        // Call LLM for filtered candidates
        if !llm_nodes.is_empty() {
            let llm_results = self.llm_strategy.evaluate_nodes(tree, &llm_nodes, context).await;

            // Map LLM results back with combined scores
            let mut llm_iter = llm_results.into_iter();
            for (idx, node_id, bm25_score) in &needs_llm {
                if candidate_indices.contains(idx) {
                    if let Some(llm_eval) = llm_iter.next() {
                        let combined_score = self.combine_scores(*bm25_score, llm_eval.score);
                        results[*idx] = NodeEvaluation {
                            score: combined_score,
                            decision: llm_eval.decision,
                            reasoning: Some(format!(
                                "Hybrid: BM25={:.2}, LLM={:.2}, Combined={:.2}",
                                bm25_score, llm_eval.score, combined_score
                            )),
                        };
                    }
                } else {
                    // Not in LLM candidates, use BM25 only
                    results[*idx] = NodeEvaluation {
                        score: *bm25_score,
                        decision: if *bm25_score > 0.5 {
                            NavigationDecision::ExploreMore
                        } else {
                            NavigationDecision::Skip
                        },
                        reasoning: Some(format!("BM25 only (filtered): {:.3}", bm25_score)),
                    };
                }
            }
        } else {
            // No LLM calls needed, use BM25 for all remaining
            for (idx, _, bm25_score) in &needs_llm {
                results[*idx] = NodeEvaluation {
                    score: *bm25_score,
                    decision: if *bm25_score > 0.5 {
                        NavigationDecision::ExploreMore
                    } else {
                        NavigationDecision::Skip
                    },
                    reasoning: Some(format!("BM25 only: {:.3}", bm25_score)),
                };
            }
        }

        results
    }

    fn name(&self) -> &'static str {
        "hybrid"
    }

    fn capabilities(&self) -> StrategyCapabilities {
        let llm_caps = self.llm_strategy.capabilities();
        StrategyCapabilities {
            uses_llm: llm_caps.uses_llm,
            uses_embeddings: false, // BM25 doesn't use embeddings
            supports_sufficiency: llm_caps.supports_sufficiency,
            typical_latency_ms: llm_caps.typical_latency_ms / 2, // Faster due to pre-filtering
        }
    }

    fn suitable_for_complexity(&self, complexity: QueryComplexity) -> bool {
        matches!(
            complexity,
            QueryComplexity::Simple | QueryComplexity::Medium | QueryComplexity::Complex
        )
    }

    fn estimate_cost(&self, node_count: usize) -> super::r#trait::StrategyCost {
        let llm_cost = self.llm_strategy.estimate_cost(node_count);

        // Estimate reduced LLM calls due to pre-filtering
        let filtered_count = ((node_count as f32 * self.config.pre_filter_ratio) as usize)
            .max(self.config.min_candidates)
            .min(self.config.max_candidates);

        // Account for auto-accept/reject
        let estimated_llm_calls = (filtered_count as f32 * 0.5) as usize;

        super::r#trait::StrategyCost {
            llm_calls: estimated_llm_calls.min(llm_cost.llm_calls),
            tokens: estimated_llm_calls * 200, // Approximate tokens per call
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = HybridConfig::default();
        assert!((config.pre_filter_ratio - 0.3).abs() < f32::EPSILON);
        assert_eq!(config.min_candidates, 2);
        assert_eq!(config.max_candidates, 5);
        assert!((config.bm25_weight - 0.4).abs() < f32::EPSILON);
        assert!((config.llm_weight - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn test_config_presets() {
        let high = HybridConfig::high_quality();
        assert!(high.max_candidates > HybridConfig::default().max_candidates);

        let low = HybridConfig::low_cost();
        assert!(low.max_candidates < HybridConfig::default().max_candidates);
    }

    #[test]
    fn test_combine_scores() {
        let strategy = HybridStrategy::new(Box::new(crate::retrieval::strategy::KeywordStrategy::new()));
        let combined = strategy.combine_scores(0.8, 0.6);

        // 0.8 * 0.4 + 0.6 * 0.6 = 0.32 + 0.36 = 0.68
        assert!((combined - 0.68).abs() < 0.01);
    }
}
