// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pure Pilot search — LLM-guided single-path tree navigation.
//!
//! At each layer, the Pilot scores all children and picks the top-1.
//! This is the most accurate (but slowest) approach: one LLM call per layer.
//! Falls back to NodeScorer when Pilot is unavailable.

use async_trait::async_trait;
use std::collections::HashSet;
use tracing::debug;

use super::super::RetrievalContext;
use super::super::types::{NavigationDecision, NavigationStep, SearchPath};
use super::{SearchConfig, SearchResult, SearchTree};
use crate::document::{DocumentTree, NodeId};
use crate::retrieval::pilot::Pilot;
use crate::retrieval::pilot::{PilotDecisionCache, score_candidates};

/// Pure Pilot search — Pilot picks the best child at each layer.
///
/// beam=1: at each level, Pilot evaluates all children and the search
/// follows only the top-ranked one. When Pilot is unavailable,
/// falls back to NodeScorer (keyword/BM25).
pub struct PurePilotSearch;

impl PurePilotSearch {
    /// Create a new Pure Pilot search.
    pub fn new() -> Self {
        Self
    }

    /// Core search logic parameterized by start node.
    async fn search_impl(
        &self,
        tree: &DocumentTree,
        context: &RetrievalContext,
        config: &SearchConfig,
        pilot: Option<&dyn Pilot>,
        start_node: NodeId,
    ) -> SearchResult {
        let mut result = SearchResult::default();
        let mut current_path = SearchPath::new();
        let mut current_node = start_node;
        let mut visited: HashSet<NodeId> = HashSet::new();
        let cache = PilotDecisionCache::new();

        debug!(
            "PurePilotSearch: query='{}', start_node={:?}, max_iterations={}, min_score={:.2}",
            context.query, start_node, config.max_iterations, config.min_score
        );

        let mut pilot_interventions = 0;

        for iteration in 0..config.max_iterations {
            result.iterations = iteration + 1;

            let children = tree.children_with_refs(current_node);

            if children.is_empty() {
                current_path.leaf = Some(current_node);
                if !config.leaf_only || tree.is_leaf(current_node) {
                    result.paths.push(current_path.clone());
                }
                break;
            }

            // Pilot as primary scorer (weight=1.0), NodeScorer as fallback.
            // Always consult Pilot — no should_intervene guard.
            let scored_children = score_candidates(
                tree,
                &children,
                &context.query,
                pilot,
                &current_path.nodes,
                &visited,
                1.0, // PurePilot: Pilot weight = 1.0
                Some(&cache),
                None, // No reasoning history tracked
            )
            .await;

            if pilot.is_some() {
                pilot_interventions += 1;
            }

            // Take only top-1
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

                current_path = current_path.extend(child_id, best_score);
                current_node = child_id;
                result.nodes_visited += 1;

                if result.paths.len() >= config.top_k {
                    break;
                }
            } else {
                current_path.leaf = Some(current_node);
                if current_path.score > 0.0 {
                    result.paths.push(current_path);
                }
                break;
            }
        }

        result.pilot_interventions = pilot_interventions;
        result
    }
}

impl Default for PurePilotSearch {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SearchTree for PurePilotSearch {
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
        "pure_pilot"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_pilot_creation() {
        let _search = PurePilotSearch::new();
    }

    #[test]
    fn test_pure_pilot_default() {
        let search = PurePilotSearch::default();
        assert_eq!(search.name(), "pure_pilot");
    }
}
