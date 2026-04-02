// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Greedy search algorithm.
//!
//! Simple depth-first search that always follows the highest-scoring child.

use async_trait::async_trait;

use crate::core::{NodeId, VectorlessTree};
use super::super::types::{NavigationDecision, NavigationStep, SearchPath};
use super::super::RetrievalContext;
use super::{SearchConfig, SearchResult, SearchTree};
use super::scorer::NodeScorer;

/// Greedy search - always follows the best single path.
///
/// Fast but may miss relevant content in other branches.
pub struct GreedySearch {
    scorer: NodeScorer,
}

impl GreedySearch {
    /// Create a new greedy search.
    pub fn new() -> Self {
        Self {
            scorer: NodeScorer::new(Default::default()),
        }
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
        tree: &VectorlessTree,
        context: &RetrievalContext,
        config: &SearchConfig,
    ) -> SearchResult {
        let mut result = SearchResult::default();
        let mut current_path = SearchPath::new();
        let mut current_node = tree.root();

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

            // Score all children
            let scored_children = self.scorer.score_and_sort(tree, &children);

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
                // Record navigation step
                let child_node = tree.get(child_id);
                result.trace.push(NavigationStep {
                    node_id: format!("{:?}", child_id),
                    title: child_node.map(|n| n.title.clone()).unwrap_or_default(),
                    score: best_score,
                    decision: NavigationDecision::GoToChild(children.iter().position(|&c| c == child_id).unwrap_or(0)),
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
                // No good children found
                current_path.leaf = Some(current_node);
                result.paths.push(current_path);
                break;
            }
        }

        result
    }

    fn name(&self) -> &str {
        "greedy"
    }
}
