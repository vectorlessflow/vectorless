// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Greedy search algorithm with Pilot integration.
//!
//! Simple depth-first search that always follows the highest-scoring child.
//! When a Pilot is provided, it can provide semantic guidance at decision points.

use async_trait::async_trait;
use tracing::{debug, trace};

use super::super::RetrievalContext;
use super::super::types::{NavigationDecision, NavigationStep, SearchPath};
use super::scorer::{NodeScorer, ScoringContext};
use super::{SearchConfig, SearchResult, SearchTree};
use crate::document::{DocumentTree, NodeId};
use crate::retrieval::pilot::{Pilot, SearchState};

/// Greedy search - always follows the best single path.
///
/// Fast but may miss relevant content in other branches.
/// When a Pilot is provided, it can guide the search at key decision points.
pub struct GreedySearch;

impl GreedySearch {
    /// Create a new greedy search.
    pub fn new() -> Self {
        Self
    }

    /// Create a scorer for the given query.
    fn create_scorer(&self, query: &str) -> NodeScorer {
        NodeScorer::new(ScoringContext::new(query))
    }

    /// Score candidates using a query-specific scorer.
    fn score_candidates_with_query(
        &self,
        tree: &DocumentTree,
        candidates: &[NodeId],
        query: &str,
    ) -> Vec<(NodeId, f32)> {
        let scorer = self.create_scorer(query);
        scorer.score_and_sort(tree, candidates)
    }

    /// Merge algorithm scores with Pilot decision.
    fn merge_with_pilot_decision(
        &self,
        tree: &DocumentTree,
        candidates: &[NodeId],
        pilot_decision: &crate::retrieval::pilot::PilotDecision,
        query: &str,
    ) -> Vec<(NodeId, f32)> {
        let scorer = self.create_scorer(query);
        let alpha = 0.4;
        let beta = 0.6 * pilot_decision.confidence;

        // Build a map from node_id to pilot score
        let mut pilot_scores: std::collections::HashMap<NodeId, f32> =
            std::collections::HashMap::new();
        for ranked in &pilot_decision.ranked_candidates {
            pilot_scores.insert(ranked.node_id, ranked.score);
        }

        // Merge scores
        let mut merged: Vec<(NodeId, f32)> = candidates
            .iter()
            .map(|&node_id| {
                let algo_score = scorer.score(tree, node_id);
                let pilot_score = pilot_scores.get(&node_id).copied().unwrap_or(0.0);

                // Weighted combination
                let final_score = if beta > 0.0 {
                    (alpha * algo_score + beta * pilot_score) / (alpha + beta)
                } else {
                    algo_score
                };

                (node_id, final_score)
            })
            .collect();

        // Sort by merged score
        merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        merged
    }
}

impl Default for GreedySearch {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SearchTree for GreedySearch {
    async fn search(
        &self,
        tree: &DocumentTree,
        context: &RetrievalContext,
        config: &SearchConfig,
        pilot: Option<&dyn Pilot>,
    ) -> SearchResult {
        let mut result = SearchResult::default();
        let mut current_path = SearchPath::new();
        let mut current_node = tree.root();
        let mut visited: std::collections::HashSet<NodeId> = std::collections::HashSet::new();

        // Track Pilot interventions
        let mut pilot_interventions = 0;

        for iteration in 0..config.max_iterations {
            result.iterations = iteration + 1;

            // Get children of current node
            let children = tree.children(current_node);

            if children.is_empty() {
                // Leaf node - add to results
                current_path.leaf = Some(current_node);
                if !config.leaf_only || tree.is_leaf(current_node) {
                    result.paths.push(current_path.clone());
                }
                break;
            }

            // ========== Pilot Integration Point ==========
            let scored_children = if let Some(p) = pilot {
                // Build search state for Pilot
                let state = SearchState::new(
                    tree,
                    &context.query,
                    &current_path.nodes,
                    &children,
                    &visited,
                );

                // Check if Pilot wants to intervene
                if p.should_intervene(&state) {
                    trace!(
                        "Pilot intervening at greedy decision point with {} candidates",
                        children.len()
                    );

                    println!("[DEBUG] GREEDY SEARCH: Pilot intervening at decision point");
                    match p.decide(&state).await {
                        decision => {
                            pilot_interventions += 1;
                            debug!(
                                "Pilot decision: confidence={}, direction={:?}",
                                decision.confidence,
                                std::mem::discriminant(&decision.direction)
                            );

                            // Merge algorithm scores with Pilot decision
                            self.merge_with_pilot_decision(
                                tree,
                                &children,
                                &decision,
                                &context.query,
                            )
                        }
                    }
                } else {
                    // No intervention, use algorithm scoring
                    self.score_candidates_with_query(tree, &children, &context.query)
                }
            } else {
                // No Pilot, use algorithm scoring
                self.score_candidates_with_query(tree, &children, &context.query)
            };
            // ==============================================

            // Find the best child that meets minimum score
            let mut best_child = None;
            let mut best_score = 0.0;

            for (child_id, score) in scored_children {
                if score >= config.min_score {
                    best_child = Some(child_id);
                    best_score = score;
                    break;
                }
            }

            if let Some(child_id) = best_child {
                visited.insert(child_id);

                // Record navigation step
                let child_node = tree.get(child_id);
                result.trace.push(NavigationStep {
                    node_id: format!("{:?}", child_id),
                    title: child_node.map(|n| n.title.clone()).unwrap_or_default(),
                    score: best_score,
                    decision: NavigationDecision::GoToChild(
                        children.iter().position(|&c| c == child_id).unwrap_or(0),
                    ),
                    depth: child_node.map(|n| n.depth).unwrap_or(0),
                });

                // Update path
                current_path = current_path.extend(child_id, best_score);
                current_node = child_id;
                result.nodes_visited += 1;

                // Check if we have enough results
                if result.paths.len() >= config.top_k {
                    break;
                }
            } else {
                // No good children found - add current path as result
                current_path.leaf = Some(current_node);
                if current_path.score > 0.0 {
                    result.paths.push(current_path);
                }
                break;
            }
        }

        // Record Pilot interventions
        result.pilot_interventions = pilot_interventions;

        result
    }

    fn name(&self) -> &'static str {
        "greedy"
    }
}
