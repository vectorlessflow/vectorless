// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Beam search algorithm with Pilot as primary scorer.
//!
//! Explores multiple paths in parallel, keeping only the top-k candidates
//! at each level. Pilot provides semantic guidance; NodeScorer is the
//! fallback when Pilot is unavailable.

use async_trait::async_trait;
use std::collections::HashSet;
use tracing::debug;

use super::super::RetrievalContext;
use super::super::types::{NavigationDecision, NavigationStep, SearchPath};
use super::pilot_scorer::{PilotDecisionCache, score_candidates};
use super::{SearchConfig, SearchResult, SearchTree};
use crate::document::{DocumentTree, NodeId};
use crate::retrieval::pilot::Pilot;

/// Beam search — explores multiple paths simultaneously.
///
/// Keeps top `beam_width` candidates at each level, providing
/// a balance between exploration and computational cost.
///
/// # Pilot Integration
///
/// Pilot is the primary scorer (weight=0.7). NodeScorer supplements
/// for candidates Pilot didn't rank. Decisions are cached by
/// (query, parent_node_id) to avoid redundant LLM calls.
pub struct BeamSearch {
    beam_width: usize,
}

impl BeamSearch {
    /// Create a new beam search with default beam width.
    pub fn new() -> Self {
        Self { beam_width: 3 }
    }

    /// Create beam search with specified width.
    pub fn with_width(width: usize) -> Self {
        Self {
            beam_width: width.max(1),
        }
    }

    /// Core beam search logic parameterized by start node.
    async fn search_impl(
        &self,
        tree: &DocumentTree,
        context: &RetrievalContext,
        config: &SearchConfig,
        pilot: Option<&dyn Pilot>,
        start_node: NodeId,
    ) -> SearchResult {
        let mut result = SearchResult::default();
        let beam_width = config.beam_width.min(self.beam_width);
        let mut visited: HashSet<NodeId> = HashSet::new();
        let cache = PilotDecisionCache::new();

        visited.insert(start_node);

        debug!(
            "BeamSearch: query='{}', start_node={:?}, beam_width={}, min_score={:.2}",
            context.query, start_node, beam_width, config.min_score
        );

        let mut pilot_interventions = 0;

        // Initialize with start_node's children
        let start_children = tree.children(start_node);
        debug!("Start node has {} children", start_children.len());

        let initial_candidates = score_candidates(
            tree,
            &start_children,
            &context.query,
            pilot,
            &[],
            &visited,
            0.7, // Beam: Pilot weight = 0.7
            Some(&cache),
        )
        .await;

        if pilot.is_some() && !start_children.is_empty() {
            pilot_interventions += 1;
        }

        let mut current_beam: Vec<SearchPath> = initial_candidates
            .into_iter()
            .map(|(node_id, score)| SearchPath::from_node(node_id, score))
            .collect();

        debug!("Initial {} candidates after scoring", current_beam.len());

        // Keep top beam_width
        current_beam.truncate(beam_width);

        for iteration in 0..config.max_iterations {
            result.iterations = iteration + 1;

            if current_beam.is_empty() {
                break;
            }

            let mut next_beam = Vec::new();

            for path in &current_beam {
                if let Some(leaf_id) = path.leaf {
                    visited.insert(leaf_id);

                    // Check if this is a leaf node
                    if tree.is_leaf(leaf_id) {
                        if path.score >= config.min_score {
                            result.paths.push(path.clone());
                        }
                        result.nodes_visited += 1;
                        continue;
                    }

                    // Expand this path
                    let children = tree.children(leaf_id);

                    let scored_children = score_candidates(
                        tree,
                        &children,
                        &context.query,
                        pilot,
                        &path.nodes,
                        &visited,
                        0.7, // Beam: Pilot weight = 0.7
                        Some(&cache),
                    )
                    .await;

                    if pilot.is_some() && !children.is_empty() {
                        pilot_interventions += 1;
                    }

                    for (child_id, child_score) in scored_children.into_iter().take(beam_width) {
                        let new_path = path.extend(child_id, child_score);

                        let child_node = tree.get(child_id);
                        result.trace.push(NavigationStep {
                            node_id: format!("{:?}", child_id),
                            title: child_node.map(|n| n.title.clone()).unwrap_or_default(),
                            score: child_score,
                            decision: NavigationDecision::GoToChild(
                                children.iter().position(|&c| c == child_id).unwrap_or(0),
                            ),
                            depth: child_node.map(|n| n.depth).unwrap_or(0),
                        });

                        next_beam.push(new_path);
                        result.nodes_visited += 1;
                    }
                }
            }

            // Sort next beam and keep top candidates
            next_beam.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            next_beam.truncate(beam_width);

            current_beam = next_beam;

            if result.paths.len() >= config.top_k {
                break;
            }
        }

        // Add any remaining paths in the beam to results
        for path in current_beam {
            if path.score >= config.min_score && result.paths.len() < config.top_k {
                result.paths.push(path);
            }
        }

        // Fallback: if no results found, add best candidates regardless of score
        if result.paths.is_empty() && config.min_score > 0.0 {
            debug!("No results above min_score, adding best candidates as fallback");
            let all_children = tree.children(start_node);
            let fallback = score_candidates(
                tree,
                &all_children,
                &context.query,
                None, // No Pilot for fallback
                &[],
                &visited,
                0.7,
                None,
            )
            .await;
            for (node_id, score) in fallback.into_iter().take(config.top_k) {
                result.paths.push(SearchPath::from_node(node_id, score));
            }
        }

        // Sort final results by score
        result.paths.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        result.paths.truncate(config.top_k);

        result.pilot_interventions = pilot_interventions;

        result
    }
}

impl Default for BeamSearch {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SearchTree for BeamSearch {
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
        "beam"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beam_search_creation() {
        let search = BeamSearch::new();
        assert_eq!(search.beam_width, 3);

        let search_wide = BeamSearch::with_width(5);
        assert_eq!(search_wide.beam_width, 5);
    }

    #[test]
    fn test_beam_search_minimum_width() {
        let search = BeamSearch::with_width(0);
        assert_eq!(search.beam_width, 1);
    }
}
