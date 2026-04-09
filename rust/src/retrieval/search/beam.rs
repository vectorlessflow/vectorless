// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Beam search algorithm with Pilot integration.
//!
//! Explores multiple paths in parallel, keeping only the top-k candidates at each level.
//! When a Pilot is provided, it can intervene at fork points to provide semantic guidance.

use async_trait::async_trait;
use std::collections::HashSet;
use tracing::{debug, trace};

use super::super::RetrievalContext;
use super::super::types::{NavigationDecision, NavigationStep, SearchPath};
use super::scorer::{NodeScorer, ScoringContext};
use super::{SearchConfig, SearchResult, SearchTree};
use crate::document::{DocumentTree, NodeId};
use crate::retrieval::pilot::{Pilot, SearchState};

/// Beam search - explores multiple paths simultaneously.
///
/// Keeps top `beam_width` candidates at each level, providing
/// a balance between exploration and computational cost.
///
/// # Pilot Integration
///
/// When a Pilot is provided, the algorithm consults it at fork points
/// (when multiple candidates are available) to get semantic guidance
/// on which branches are most relevant to the query.
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
    ///
    /// Uses weighted combination: `final = α * algo + β * pilot`
    /// where α = 0.4 and β = 0.6 * confidence
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

    /// Core beam search logic parameterized by start node.
    ///
    /// This is the shared implementation used by both `search` (starts from root)
    /// and `search_from` (starts from an arbitrary node).
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

        // Mark start_node as visited so we don't go back up
        visited.insert(start_node);

        debug!(
            "BeamSearch: query='{}', start_node={:?}, beam_width={}, min_score={:.2}",
            context.query, start_node, beam_width, config.min_score
        );

        // Track Pilot interventions
        let mut pilot_interventions = 0;

        // Initialize with start_node's children
        let start_children = tree.children(start_node);
        debug!("Start node has {} children", start_children.len());

        // Check if Pilot wants to guide the start
        let initial_candidates = if let Some(p) = pilot {
            debug!(
                "BeamSearch: Pilot is available, name={}, guide_at_start={}",
                p.name(),
                p.config().guide_at_start
            );
            if p.config().guide_at_start {
                if let Some(guidance) = p.guide_start(tree, &context.query).await {
                    debug!(
                        "Pilot provided start guidance with confidence {}",
                        guidance.confidence
                    );
                    pilot_interventions += 1;

                    if guidance.has_candidates() {
                        self.merge_with_pilot_decision(
                            tree,
                            &start_children,
                            &guidance,
                            &context.query,
                        )
                    } else {
                        self.score_candidates_with_query(tree, &start_children, &context.query)
                    }
                } else {
                    self.score_candidates_with_query(tree, &start_children, &context.query)
                }
            } else {
                self.score_candidates_with_query(tree, &start_children, &context.query)
            }
        } else {
            self.score_candidates_with_query(tree, &start_children, &context.query)
        };

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

                    // ========== Pilot Intervention Point ==========
                    let scored_children = if let Some(p) = pilot {
                        let state = SearchState::new(
                            tree,
                            &context.query,
                            &path.nodes,
                            &children,
                            &visited,
                        );

                        if p.should_intervene(&state) {
                            trace!(
                                "Pilot intervening at fork with {} candidates",
                                children.len()
                            );

                            match p.decide(&state).await {
                                decision => {
                                    pilot_interventions += 1;
                                    debug!(
                                        "Pilot decision: confidence={}, direction={:?}",
                                        decision.confidence,
                                        std::mem::discriminant(&decision.direction)
                                    );

                                    self.merge_with_pilot_decision(
                                        tree,
                                        &children,
                                        &decision,
                                        &context.query,
                                    )
                                }
                            }
                        } else {
                            self.score_candidates_with_query(tree, &children, &context.query)
                        }
                    } else {
                        self.score_candidates_with_query(tree, &children, &context.query)
                    };
                    // ==============================================

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
            let all_candidates =
                self.score_candidates_with_query(tree, &tree.children(start_node), &context.query);
            for (node_id, score) in all_candidates.into_iter().take(config.top_k) {
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
        self.search_impl(tree, context, config, pilot, tree.root()).await
    }

    async fn search_from(
        &self,
        tree: &DocumentTree,
        context: &RetrievalContext,
        config: &SearchConfig,
        pilot: Option<&dyn Pilot>,
        start_node: NodeId,
    ) -> SearchResult {
        self.search_impl(tree, context, config, pilot, start_node).await
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
