// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Monte Carlo Tree Search (MCTS) with Pilot-provided priors.
//!
//! Uses UCT (Upper Confidence Bound for Trees) to balance exploration
//! and exploitation. Pilot provides prior scores for the UCT formula,
//! and guides the simulation (rollout) phase. NodeScorer is the fallback
//! when Pilot is unavailable.
//!
//! # Async
//!
//! Both selection and simulation phases are async because Pilot.decide()
//! requires an LLM call. Pilot decisions are cached by (query, parent_node_id)
//! so repeated visits to the same node don't trigger redundant LLM calls.

use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use tracing::debug;

use super::super::RetrievalContext;
use super::super::types::{NavigationDecision, NavigationStep, SearchPath};
use super::{SearchConfig, SearchResult, SearchTree};
use crate::document::{DocumentTree, NodeId};
use crate::retrieval::pilot::Pilot;
use crate::retrieval::pilot::{NodeScorer, PilotDecisionCache, ScoringContext, score_candidates};

/// Statistics for a node in MCTS.
#[derive(Debug, Clone, Default)]
struct NodeStats {
    /// Number of visits.
    visits: usize,
    /// Cumulative score.
    total_score: f32,
}

/// MCTS search with Pilot integration.
///
/// Pilot provides prior scores that seed the UCT formula. This gives
/// MCTS semantic guidance while preserving the exploration/exploitation
/// balance. NodeScorer is used as fallback when Pilot is unavailable.
pub struct MctsSearch {
    /// Exploration constant for UCT.
    exploration_weight: f32,
}

impl MctsSearch {
    /// Create a new MCTS search.
    pub fn new() -> Self {
        Self {
            exploration_weight: 1.414, // sqrt(2), classic UCT default
        }
    }

    /// Set exploration weight.
    pub fn with_exploration(mut self, weight: f32) -> Self {
        self.exploration_weight = weight;
        self
    }

    /// Calculate UCT score for a child node.
    ///
    /// `prior_score` comes from Pilot (or NodeScorer fallback).
    fn uct_score(&self, child_stats: &NodeStats, parent_visits: usize, prior_score: f32) -> f32 {
        if child_stats.visits == 0 {
            // Unvisited nodes get high priority + prior bonus
            return f32::INFINITY;
        }

        let exploitation = child_stats.total_score / child_stats.visits as f32;
        let exploration = self.exploration_weight * (parent_visits as f32).ln().sqrt()
            / child_stats.visits as f32;

        // Blend exploitation with Pilot prior
        0.5 * (exploitation + prior_score) + exploration
    }

    /// Select best child using UCT with Pilot priors.
    ///
    /// When Pilot is available, fetches priors via the cache.
    /// Falls back to NodeScorer when Pilot is unavailable.
    async fn select_child(
        &self,
        tree: &DocumentTree,
        context: &RetrievalContext,
        node_id: NodeId,
        stats: &HashMap<NodeId, NodeStats>,
        pilot: Option<&dyn Pilot>,
        cache: &PilotDecisionCache,
        visited: &HashSet<NodeId>,
    ) -> Option<(NodeId, f32)> {
        let children = tree.children_with_refs(node_id);
        if children.is_empty() {
            return None;
        }

        let parent_stats = stats.get(&node_id).cloned().unwrap_or_default();
        let parent_visits = parent_stats.visits.max(1);

        // Get Pilot priors for all children (cached)
        let priors = score_candidates(
            tree,
            &children,
            &context.query,
            pilot,
            &[node_id], // simplified path for UCT context
            visited,
            0.5, // MCTS prior: balanced Pilot/Scorer
            Some(cache),
            None, // No reasoning history tracked
        )
        .await;

        // Build prior map
        let prior_map: HashMap<NodeId, f32> = priors.into_iter().collect();

        let mut best_child = None;
        let mut best_score = f32::NEG_INFINITY;

        for &child_id in &children {
            let prior = prior_map.get(&child_id).copied().unwrap_or_else(|| {
                let scorer = NodeScorer::new(ScoringContext::new(&context.query));
                scorer.score(tree, child_id)
            });
            let child_stats = stats.get(&child_id).cloned().unwrap_or_default();
            let uct = self.uct_score(&child_stats, parent_visits, prior);

            if uct > best_score {
                best_score = uct;
                best_child = Some((child_id, prior));
            }
        }

        best_child
    }

    /// Simulate a rollout from a node using Pilot-guided greedy descent.
    ///
    /// When Pilot is available, each layer picks the top-1 Pilot-scored child.
    /// Falls back to NodeScorer when Pilot is unavailable.
    async fn simulate(
        &self,
        tree: &DocumentTree,
        context: &RetrievalContext,
        node_id: NodeId,
        max_depth: usize,
        pilot: Option<&dyn Pilot>,
        cache: &PilotDecisionCache,
        visited: &HashSet<NodeId>,
    ) -> f32 {
        let mut current = node_id;
        let mut depth = 0;
        let mut path = vec![node_id];
        let mut total_score = 0.0f32;
        let mut count = 0;

        // Initial score
        let scorer = NodeScorer::new(ScoringContext::new(&context.query));
        total_score += scorer.score(tree, current);
        count += 1;

        while depth < max_depth {
            let children = tree.children_with_refs(current);
            if children.is_empty() {
                break;
            }

            // Use Pilot for greedy descent (cached)
            let scored = score_candidates(
                tree,
                &children,
                &context.query,
                pilot,
                &path,
                visited,
                0.5, // MCTS simulation: balanced
                Some(cache),
                None, // No reasoning history tracked
            )
            .await;

            if let Some(&(child_id, score)) = scored.first() {
                total_score += score;
                path.push(child_id);
                current = child_id;
            } else {
                break;
            }
            depth += 1;
            count += 1;
        }

        total_score / count.max(1) as f32
    }

    /// Backpropagate score up the tree.
    fn backpropagate(&self, stats: &mut HashMap<NodeId, NodeStats>, path: &[NodeId], score: f32) {
        for &node_id in path {
            let node_stats = stats.entry(node_id).or_default();
            node_stats.visits += 1;
            node_stats.total_score += score;
        }
    }

    /// Core MCTS logic parameterized by start node.
    async fn search_impl(
        &self,
        tree: &DocumentTree,
        context: &RetrievalContext,
        config: &SearchConfig,
        pilot: Option<&dyn Pilot>,
        start_node: NodeId,
    ) -> SearchResult {
        let mut result = SearchResult::default();
        let mut stats: HashMap<NodeId, NodeStats> = HashMap::new();
        let cache = PilotDecisionCache::new();
        let visited: HashSet<NodeId> = HashSet::new();

        // Initialize root stats
        stats.insert(start_node, NodeStats::default());

        debug!(
            "MctsSearch: query='{}', start_node={:?}, max_iterations={}, exploration={:.2}",
            context.query, start_node, config.max_iterations, self.exploration_weight
        );

        let mut pilot_interventions = 0;

        for iteration in 0..config.max_iterations {
            result.iterations = iteration + 1;

            // === Selection phase: traverse tree using UCT ===
            let mut path = vec![start_node];
            let mut current = start_node;

            while !tree.is_leaf(current) {
                if let Some((child_id, _score)) = self
                    .select_child(tree, context, current, &stats, pilot, &cache, &visited)
                    .await
                {
                    path.push(child_id);
                    current = child_id;
                    if pilot.is_some() {
                        pilot_interventions += 1;
                    }
                } else {
                    break;
                }
            }

            result.nodes_visited += path.len();

            // === Simulation phase: Pilot-guided rollout ===
            let leaf = *path.last().unwrap_or(&start_node);
            let sim_score = self
                .simulate(tree, context, leaf, 5, pilot, &cache, &visited)
                .await;

            if pilot.is_some() {
                pilot_interventions += 1;
            }

            // === Backpropagation phase ===
            self.backpropagate(&mut stats, &path, sim_score);

            // Record trace for the last node
            if let Some(&last_id) = path.last() {
                let node = tree.get(last_id);
                result.trace.push(NavigationStep {
                    node_id: format!("{:?}", last_id),
                    title: node.map(|n| n.title.clone()).unwrap_or_default(),
                    score: sim_score,
                    decision: NavigationDecision::ExploreMore,
                    depth: node.map(|n| n.depth).unwrap_or(0),
                });
            }

            // Periodically extract paths (every 10 iterations)
            if iteration > 0 && iteration % 10 == 0 {
                self.extract_paths(
                    tree,
                    start_node,
                    &stats,
                    config.min_score,
                    config.top_k,
                    &mut result,
                );
            }
        }

        // Final extraction of best paths
        self.extract_paths(
            tree,
            start_node,
            &stats,
            config.min_score,
            config.top_k,
            &mut result,
        );

        result.pilot_interventions = pilot_interventions;
        result
    }

    /// Extract best paths from MCTS statistics.
    fn extract_paths(
        &self,
        tree: &DocumentTree,
        root: NodeId,
        stats: &HashMap<NodeId, NodeStats>,
        min_score: f32,
        top_k: usize,
        result: &mut SearchResult,
    ) {
        let root_children = tree.children_with_refs(root);
        let mut scored_children: Vec<_> = root_children
            .iter()
            .filter_map(|&child_id| {
                stats.get(&child_id).map(|s| {
                    let avg_score = if s.visits > 0 {
                        s.total_score / s.visits as f32
                    } else {
                        0.0
                    };
                    (child_id, avg_score)
                })
            })
            .collect();

        scored_children.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Clear existing paths and re-extract
        result.paths = scored_children
            .into_iter()
            .filter(|(_, score)| *score >= min_score)
            .take(top_k)
            .map(|(node_id, score)| SearchPath::from_node(node_id, score))
            .collect();
    }
}

impl Default for MctsSearch {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SearchTree for MctsSearch {
    async fn search(
        &self,
        tree: &DocumentTree,
        context: &RetrievalContext,
        config: &SearchConfig,
        pilot: Option<&dyn Pilot>,
    ) -> SearchResult {
        self.search_impl(tree, context, config, pilot, tree.root())
            .await
    }

    async fn search_from(
        &self,
        tree: &DocumentTree,
        context: &RetrievalContext,
        config: &SearchConfig,
        pilot: Option<&dyn Pilot>,
        start_node: NodeId,
    ) -> SearchResult {
        self.search_impl(tree, context, config, pilot, start_node)
            .await
    }

    fn name(&self) -> &'static str {
        "mcts"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcts_creation() {
        let search = MctsSearch::new();
        assert!((search.exploration_weight - 1.414).abs() < 0.01);
    }

    #[test]
    fn test_mcts_custom_exploration() {
        let search = MctsSearch::new().with_exploration(2.0);
        assert!((search.exploration_weight - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_uct_unvisited() {
        let search = MctsSearch::new();
        let stats = NodeStats::default();
        let score = search.uct_score(&stats, 10, 0.5);
        assert!(score.is_infinite());
    }

    #[test]
    fn test_uct_visited() {
        let search = MctsSearch::new();
        let stats = NodeStats {
            visits: 5,
            total_score: 3.0,
        };
        let score = search.uct_score(&stats, 20, 0.8);
        assert!(score.is_finite());
        assert!(score > 0.0);
    }
}
