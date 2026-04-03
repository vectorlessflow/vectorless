// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Monte Carlo Tree Search (MCTS) algorithm.
//!
//! Balances exploration and exploitation using UCT formula.

use async_trait::async_trait;
use std::collections::HashMap;

use crate::config::StrategyConfig;
use crate::core::{NodeId, VectorlessTree};
use super::super::types::{NavigationDecision, NavigationStep, SearchPath};
use super::super::RetrievalContext;
use super::{SearchConfig, SearchResult, SearchTree};
use super::scorer::NodeScorer;

/// Statistics for a node in MCTS.
#[derive(Debug, Clone, Default)]
struct NodeStats {
    /// Number of visits.
    visits: usize,
    /// Cumulative score.
    total_score: f32,
}

/// Monte Carlo Tree Search implementation.
///
/// Uses UCT (Upper Confidence Bound for Trees) to balance
/// exploration of new paths with exploitation of promising ones.
pub struct MctsSearch {
    scorer: NodeScorer,
    /// Exploration constant for UCT.
    exploration_weight: f32,
}

impl MctsSearch {
    /// Create a new MCTS search.
    pub fn new() -> Self {
        Self::with_config(&StrategyConfig::default())
    }

    /// Create with configuration.
    pub fn with_config(config: &StrategyConfig) -> Self {
        Self {
            scorer: NodeScorer::new(Default::default()),
            exploration_weight: config.exploration_weight,
        }
    }

    /// Set exploration weight.
    pub fn with_exploration(mut self, weight: f32) -> Self {
        self.exploration_weight = weight;
        self
    }

    /// Calculate UCT score for a child node.
    fn uct_score(
        &self,
        child_stats: &NodeStats,
        parent_visits: usize,
        prior_score: f32,
    ) -> f32 {
        if child_stats.visits == 0 {
            // Unvisited nodes get high priority
            return f32::INFINITY;
        }

        let exploitation = child_stats.total_score / child_stats.visits as f32;
        let exploration = self.exploration_weight
            * (parent_visits as f32).ln().sqrt()
            / child_stats.visits as f32;

        // Combine with prior score from scorer
        0.5 * (exploitation + prior_score) + exploration
    }

    /// Select best child using UCT.
    fn select_child(
        &self,
        tree: &VectorlessTree,
        node_id: NodeId,
        stats: &HashMap<NodeId, NodeStats>,
    ) -> Option<(NodeId, f32)> {
        let children = tree.children(node_id);
        if children.is_empty() {
            return None;
        }

        let parent_stats = stats.get(&node_id).cloned().unwrap_or_default();
        let parent_visits = parent_stats.visits.max(1);

        let mut best_child = None;
        let mut best_score = f32::NEG_INFINITY;

        for &child_id in &children {
            let prior_score = self.scorer.score(tree, child_id);
            let child_stats = stats.get(&child_id).cloned().unwrap_or_default();
            let uct = self.uct_score(&child_stats, parent_visits, prior_score);

            if uct > best_score {
                best_score = uct;
                best_child = Some((child_id, prior_score));
            }
        }

        best_child
    }

    /// Simulate a random rollout from a node.
    fn simulate(&self, tree: &VectorlessTree, node_id: NodeId, max_depth: usize) -> f32 {
        let mut current = node_id;
        let mut depth = 0;
        let mut total_score = self.scorer.score(tree, current);

        while depth < max_depth {
            let children = tree.children(current);
            if children.is_empty() {
                break;
            }

            // Random selection (or use scorer for semi-random)
            let scored = self.scorer.score_and_sort(tree, &children);
            if let Some((child_id, score)) = scored.first() {
                total_score += score;
                current = *child_id;
            } else {
                break;
            }
            depth += 1;
        }

        total_score / (depth + 1).max(1) as f32
    }

    /// Backpropagate score up the tree.
    fn backpropagate(
        &self,
        stats: &mut HashMap<NodeId, NodeStats>,
        path: &[NodeId],
        score: f32,
    ) {
        for &node_id in path {
            let node_stats = stats.entry(node_id).or_default();
            node_stats.visits += 1;
            node_stats.total_score += score;
        }
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
        tree: &VectorlessTree,
        context: &RetrievalContext,
        config: &SearchConfig,
    ) -> SearchResult {
        let mut result = SearchResult::default();
        let mut stats: HashMap<NodeId, NodeStats> = HashMap::new();
        let root = tree.root();

        // Initialize root stats
        stats.insert(root, NodeStats::default());

        for iteration in 0..config.max_iterations {
            result.iterations = iteration + 1;

            // Selection phase - traverse tree using UCT
            let mut path = vec![root];
            let mut current = root;

            while !tree.is_leaf(current) {
                if let Some((child_id, score)) = self.select_child(tree, current, &stats) {
                    path.push(child_id);
                    current = child_id;
                } else {
                    break;
                }
            }

            result.nodes_visited += path.len();

            // Simulation phase - random rollout
            let leaf = *path.last().unwrap_or(&root);
            let sim_score = self.simulate(tree, leaf, 5);

            // Backpropagation phase
            self.backpropagate(&mut stats, &path, sim_score);

            // Record trace for the last node in path
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

            // Check if we have enough visits to extract paths
            if iteration > 0 && iteration % 10 == 0 {
                // Extract best paths from visited nodes
                let root_children = tree.children(root);
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

                for (child_id, score) in scored_children.iter().take(config.top_k) {
                    if *score >= config.min_score {
                        result.paths.push(SearchPath::from_node(*child_id, *score));
                    }
                }
            }
        }

        // Final extraction of best paths
        let root_children = tree.children(root);
        let mut final_paths: Vec<_> = root_children
            .iter()
            .filter_map(|&child_id| {
                stats.get(&child_id).map(|s| {
                    let avg_score = if s.visits > 0 {
                        s.total_score / s.visits as f32
                    } else {
                        self.scorer.score(tree, child_id)
                    };
                    SearchPath::from_node(child_id, avg_score)
                })
            })
            .collect();

        final_paths.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        final_paths.truncate(config.top_k);

        result.paths = final_paths
            .into_iter()
            .filter(|p| p.score >= config.min_score)
            .collect();

        result
    }

    fn name(&self) -> &str {
        "mcts"
    }
}
