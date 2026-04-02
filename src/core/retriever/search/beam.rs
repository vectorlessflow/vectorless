// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Beam search algorithm.
//!
//! Explores multiple paths in parallel, keeping only the top-k candidates at each level.

use async_trait::async_trait;

use crate::core::VectorlessTree;
use super::super::types::{NavigationDecision, NavigationStep, SearchPath};
use super::super::RetrievalContext;
use super::{SearchConfig, SearchResult, SearchTree};
use super::scorer::NodeScorer;

/// Beam search - explores multiple paths simultaneously.
///
/// Keeps top `beam_width` candidates at each level, providing
/// a balance between exploration and computational cost.
pub struct BeamSearch {
    scorer: NodeScorer,
    beam_width: usize,
}

impl BeamSearch {
    /// Create a new beam search with default beam width.
    pub fn new() -> Self {
        Self {
            scorer: NodeScorer::new(Default::default()),
            beam_width: 3,
        }
    }

    /// Create beam search with specified width.
    pub fn with_width(width: usize) -> Self {
        Self {
            scorer: NodeScorer::new(Default::default()),
            beam_width: width.max(1),
        }
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
        tree: &VectorlessTree,
        context: &RetrievalContext,
        config: &SearchConfig,
    ) -> SearchResult {
        let mut result = SearchResult::default();
        let beam_width = config.beam_width.min(self.beam_width);

        // Initialize with root's children
        let root_children = tree.children(tree.root());
        let mut current_beam: Vec<SearchPath> = root_children
            .iter()
            .map(|&child_id| {
                let score = self.scorer.score(tree, child_id);
                SearchPath::from_node(child_id, score)
            })
            .collect();

        // Sort by score and keep top beam_width
        current_beam.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        current_beam.truncate(beam_width);

        for iteration in 0..config.max_iterations {
            result.iterations = iteration + 1;

            if current_beam.is_empty() {
                break;
            }

            let mut next_beam = Vec::new();

            for path in &current_beam {
                if let Some(leaf_id) = path.leaf {
                    // Check if this is a leaf node
                    if tree.is_leaf(leaf_id) {
                        // Add to final results
                        if path.score >= config.min_score {
                            result.paths.push(path.clone());
                        }
                        result.nodes_visited += 1;
                        continue;
                    }

                    // Expand this path
                    let children = tree.children(leaf_id);
                    let scored_children = self.scorer.score_and_sort(tree, &children);

                    for (child_id, child_score) in scored_children.into_iter().take(beam_width) {
                        let new_path = path.extend(child_id, child_score);

                        // Record trace
                        let child_node = tree.get(child_id);
                        result.trace.push(NavigationStep {
                            node_id: format!("{:?}", child_id),
                            title: child_node.map(|n| n.title.clone()).unwrap_or_default(),
                            score: child_score,
                            decision: NavigationDecision::GoToChild(
                                children.iter().position(|&c| c == child_id).unwrap_or(0)
                            ),
                            depth: child_node.map(|n| n.depth).unwrap_or(0),
                        });

                        next_beam.push(new_path);
                        result.nodes_visited += 1;
                    }
                }
            }

            // Sort next beam and keep top candidates
            next_beam.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            next_beam.truncate(beam_width);

            current_beam = next_beam;

            // Check if we have enough results
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

        // Sort final results by score
        result.paths.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        result.paths.truncate(config.top_k);

        result
    }

    fn name(&self) -> &str {
        "beam"
    }
}
