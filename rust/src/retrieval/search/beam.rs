// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Beam search algorithm with Pilot as primary scorer and backtracking support.
//!
//! Explores multiple paths in parallel, keeping only the top-k candidates
//! at each level. Pilot provides semantic guidance; NodeScorer is the
//! fallback when Pilot is unavailable.
//!
//! # Backtracking
//!
//! When the main beam exhausts all paths without finding enough results,
//! the search pops entries from a fallback stack and tries alternative
//! branches. This prevents the search from getting stuck in dead ends
//! caused by Pilot misjudgments at early layers.

use async_trait::async_trait;
use std::collections::HashSet;
use tracing::debug;

use super::super::RetrievalContext;
use super::super::types::{NavigationDecision, NavigationStep, SearchPath};
use super::{SearchConfig, SearchResult, SearchTree};
use crate::document::{DocumentTree, NodeId};
use crate::retrieval::pilot::{Pilot, SearchState};
use crate::retrieval::pilot::{PilotDecisionCache, score_candidates, score_candidates_detailed};

/// Maximum entries in the fallback stack relative to beam width.
const FALLBACK_STACK_MULTIPLIER: usize = 3;

/// An entry in the fallback stack representing a viable alternative path
/// that was truncated from the main beam.
#[derive(Debug, Clone)]
struct FallbackEntry {
    /// The alternative search path.
    path: SearchPath,
    /// Score when this path was shelved.
    score: f32,
}

/// Beam search — explores multiple paths simultaneously with backtracking.
///
/// Keeps top `beam_width` candidates at each level, providing
/// a balance between exploration and computational cost.
///
/// # Pilot Integration
///
/// Pilot is the primary scorer (weight=0.7). NodeScorer supplements
/// for candidates Pilot didn't rank. Decisions are cached by
/// (query, parent_node_id) to avoid redundant LLM calls.
///
/// # Backtracking
///
/// Paths truncated from the beam that still have reasonable scores
/// are kept in a fallback stack. When the main beam empties without
/// finding enough results, the search pops from the fallback stack,
/// calls `Pilot::guide_backtrack()` for re-guidance, and continues
/// from the alternative path.
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

    /// Push a path into the fallback stack if it meets the score threshold.
    fn push_fallback(
        fallback_stack: &mut Vec<FallbackEntry>,
        entry: FallbackEntry,
        min_score: f32,
        fallback_score_ratio: f32,
        max_size: usize,
    ) {
        let threshold = min_score * fallback_score_ratio;
        if entry.score < threshold {
            return;
        }

        // Evict lowest-score entry if at capacity
        if fallback_stack.len() >= max_size {
            if let Some(min_idx) = fallback_stack
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    a.score
                        .partial_cmp(&b.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
            {
                if entry.score > fallback_stack[min_idx].score {
                    fallback_stack.swap_remove(min_idx);
                } else {
                    return; // New entry isn't better than worst in stack
                }
            }
        }

        fallback_stack.push(entry);
    }

    /// Pop the highest-score entry from the fallback stack.
    fn pop_fallback(fallback_stack: &mut Vec<FallbackEntry>) -> Option<FallbackEntry> {
        if fallback_stack.is_empty() {
            return None;
        }
        // Find and remove the highest-score entry
        let max_idx = fallback_stack
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)?;
        Some(fallback_stack.swap_remove(max_idx))
    }

    /// Attempt backtracking by popping from the fallback stack and
    /// consulting Pilot for re-guidance.
    async fn try_backtrack(
        &self,
        tree: &DocumentTree,
        context: &RetrievalContext,
        pilot: Option<&dyn Pilot>,
        cache: &PilotDecisionCache,
        visited: &HashSet<NodeId>,
        fallback_stack: &mut Vec<FallbackEntry>,
        result: &mut SearchResult,
        pilot_interventions: &mut usize,
    ) -> Option<SearchPath> {
        let entry = Self::pop_fallback(fallback_stack)?;
        let dead_end_title = entry
            .path
            .leaf
            .and_then(|id| tree.get(id))
            .map(|n| n.title.clone())
            .unwrap_or_else(|| "unknown".to_string());

        debug!(
            "Backtracking: trying alternative path (score={:.2}, dead_end='{}')",
            entry.score, dead_end_title
        );

        // Record backtrack in trace
        result.trace.push(NavigationStep {
            node_id: format!("{:?}", entry.path.leaf),
            title: dead_end_title.clone(),
            score: entry.score,
            decision: NavigationDecision::BacktrackFrom(dead_end_title),
            depth: entry.path.nodes.len(),
        });

        // Consult Pilot for re-guidance at the backtracking point
        if let Some(p) = pilot {
            // Get siblings of the dead-end node (alternatives at the same level)
            let parent_node = if entry.path.nodes.len() >= 2 {
                entry.path.nodes[entry.path.nodes.len() - 2]
            } else {
                tree.root()
            };
            let siblings = tree.children(parent_node);
            let unvisited_siblings: Vec<NodeId> = siblings
                .into_iter()
                .filter(|id| !visited.contains(id))
                .collect();

            if !unvisited_siblings.is_empty() {
                let path_ref = &entry.path.nodes[..];
                let state = SearchState {
                    tree,
                    query: &context.query,
                    path: path_ref,
                    candidates: &unvisited_siblings,
                    visited,
                    depth: entry.path.nodes.len(),
                    iteration: result.iterations,
                    best_score: result.paths.iter().map(|p| p.score).fold(0.0f32, f32::max),
                    is_backtracking: true,
                    step_reasons: Some(&entry.path.step_reasons),
                };

                if let Some(decision) = p.guide_backtrack(&state).await {
                    *pilot_interventions += 1;

                    // Use Pilot's ranked candidates to pick the best alternative
                    if let Some(top) = decision.top_candidate() {
                        let new_path = entry.path.extend(top.node_id, top.score);
                        let child_node = tree.get(top.node_id);
                        result.trace.push(NavigationStep {
                            node_id: format!("{:?}", top.node_id),
                            title: child_node.map(|n| n.title.clone()).unwrap_or_default(),
                            score: top.score,
                            decision: NavigationDecision::GoToChild(
                                unvisited_siblings
                                    .iter()
                                    .position(|&c| c == top.node_id)
                                    .unwrap_or(0),
                            ),
                            depth: child_node.map(|n| n.depth).unwrap_or(0),
                        });
                        result.nodes_visited += 1;
                        debug!(
                            "Pilot re-guided to '{}' (score={:.2})",
                            child_node.map(|n| n.title.clone()).unwrap_or_default(),
                            top.score
                        );
                        return Some(new_path);
                    }
                }
            }
        }

        // No Pilot guidance or Pilot returned None — use the path as-is
        // (continue expanding from where it was shelved)
        debug!("No Pilot guidance during backtrack, using shelved path as-is");
        Some(entry.path)
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
        let max_fallback_size = beam_width * FALLBACK_STACK_MULTIPLIER;
        let mut visited: HashSet<NodeId> = HashSet::new();
        let cache = PilotDecisionCache::new();

        visited.insert(start_node);

        debug!(
            "BeamSearch: query='{}', start_node={:?}, beam_width={}, min_score={:.2}, max_backtracks={}",
            context.query, start_node, beam_width, config.min_score, config.max_backtracks
        );

        let mut pilot_interventions = 0;
        let mut backtrack_count = 0;

        // Fallback stack holds viable paths truncated from the beam
        let mut fallback_stack: Vec<FallbackEntry> = Vec::new();

        // Initialize with start_node's children (includes resolved cross-references)
        let start_children = tree.children_with_refs(start_node);
        debug!("Start node has {} children", start_children.len());

        let initial_candidates = score_candidates_detailed(
            tree,
            &start_children,
            &context.query,
            pilot,
            &[],
            &visited,
            0.7, // Beam: Pilot weight = 0.7
            Some(&cache),
            None, // No reasoning history at start
        )
        .await;

        if pilot.is_some() && !start_children.is_empty() {
            pilot_interventions += 1;
        }

        // Split initial candidates into beam and fallback
        let mut sorted_initial: Vec<_> = initial_candidates
            .into_iter()
            .map(|s| {
                let mut path = SearchPath::from_node(s.node_id, s.score);
                // Record reason for initial selection
                if let Some(reason) = s.reason {
                    path.step_reasons = vec![Some(reason)];
                }
                path
            })
            .collect();
        sorted_initial.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut current_beam: Vec<SearchPath> =
            sorted_initial.iter().take(beam_width).cloned().collect();

        // Remaining candidates go to fallback stack
        for path in sorted_initial.iter().skip(beam_width) {
            Self::push_fallback(
                &mut fallback_stack,
                FallbackEntry {
                    path: path.clone(),
                    score: path.score,
                },
                config.min_score,
                config.fallback_score_ratio,
                max_fallback_size,
            );
        }

        debug!(
            "Initial beam={}, fallback_stack={}",
            current_beam.len(),
            fallback_stack.len()
        );

        for iteration in 0..config.max_iterations {
            result.iterations = iteration + 1;

            // === BACKTRACKING CHECK ===
            // If beam is empty but we have fallback entries and haven't
            // found enough results, try backtracking.
            if current_beam.is_empty() && result.paths.len() < config.top_k {
                if backtrack_count < config.max_backtracks {
                    if let Some(new_path) = self
                        .try_backtrack(
                            tree,
                            context,
                            pilot,
                            &cache,
                            &visited,
                            &mut fallback_stack,
                            &mut result,
                            &mut pilot_interventions,
                        )
                        .await
                    {
                        backtrack_count += 1;
                        current_beam = vec![new_path];
                        debug!(
                            "Backtrack #{}: injected path into beam (remaining fallback={})",
                            backtrack_count,
                            fallback_stack.len()
                        );
                        // Continue the search from this path
                        continue;
                    }
                }
                // No more fallback options or max backtracks reached
                break;
            }

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

                    // Expand this path (includes resolved cross-references)
                    let children = tree.children_with_refs(leaf_id);

                    let scored_children = score_candidates_detailed(
                        tree,
                        &children,
                        &context.query,
                        pilot,
                        &path.nodes,
                        &visited,
                        0.7, // Beam: Pilot weight = 0.7
                        Some(&cache),
                        Some(&path.step_reasons),
                    )
                    .await;

                    if pilot.is_some() && !children.is_empty() {
                        pilot_interventions += 1;
                    }

                    for sc in scored_children.into_iter().take(beam_width) {
                        let new_path = if let Some(ref reason) = sc.reason {
                            path.extend_with_reason(sc.node_id, sc.score, reason)
                        } else {
                            path.extend(sc.node_id, sc.score)
                        };

                        let child_node = tree.get(sc.node_id);
                        result.trace.push(NavigationStep {
                            node_id: format!("{:?}", sc.node_id),
                            title: child_node.map(|n| n.title.clone()).unwrap_or_default(),
                            score: sc.score,
                            decision: NavigationDecision::GoToChild(
                                children.iter().position(|&c| c == sc.node_id).unwrap_or(0),
                            ),
                            depth: child_node.map(|n| n.depth).unwrap_or(0),
                        });

                        next_beam.push(new_path);
                        result.nodes_visited += 1;
                    }
                }
            }

            // Sort next beam and split into beam + fallback
            next_beam.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Keep top beam_width in the beam, shelve the rest
            let mut beam_candidates = next_beam;
            let overflow: Vec<SearchPath> =
                beam_candidates.split_off(beam_width.min(beam_candidates.len()));

            for path in overflow {
                let score = path.score;
                Self::push_fallback(
                    &mut fallback_stack,
                    FallbackEntry { path, score },
                    config.min_score,
                    config.fallback_score_ratio,
                    max_fallback_size,
                );
            }

            current_beam = beam_candidates;

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
            let all_children = tree.children_with_refs(start_node);
            let fallback = score_candidates(
                tree,
                &all_children,
                &context.query,
                None, // No Pilot for fallback
                &[],
                &visited,
                0.7,
                None,
                None, // No reasoning history for fallback
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

        debug!(
            "BeamSearch complete: paths={}, iterations={}, backtracks={}, pilot_interventions={}",
            result.paths.len(),
            result.iterations,
            backtrack_count,
            pilot_interventions
        );

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
    use crate::document::TreeNode;
    use indextree::Arena;

    /// Helper to create a NodeId from an Arena for tests.
    fn make_node_id(arena: &mut Arena<TreeNode>) -> NodeId {
        NodeId(arena.new_node(TreeNode::default()))
    }

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

    #[test]
    fn test_fallback_push_and_pop() {
        let mut arena = Arena::new();
        let id0 = make_node_id(&mut arena);
        let id1 = make_node_id(&mut arena);
        let id2 = make_node_id(&mut arena);
        let mut stack = Vec::new();

        BeamSearch::push_fallback(
            &mut stack,
            FallbackEntry {
                path: SearchPath::from_node(id0, 0.3),
                score: 0.3,
            },
            0.1,
            0.5,
            100,
        );
        BeamSearch::push_fallback(
            &mut stack,
            FallbackEntry {
                path: SearchPath::from_node(id1, 0.7),
                score: 0.7,
            },
            0.1,
            0.5,
            100,
        );
        BeamSearch::push_fallback(
            &mut stack,
            FallbackEntry {
                path: SearchPath::from_node(id2, 0.5),
                score: 0.5,
            },
            0.1,
            0.5,
            100,
        );

        assert_eq!(stack.len(), 3);

        // Pop should return highest score (0.7)
        let popped = BeamSearch::pop_fallback(&mut stack);
        assert!(popped.is_some());
        assert!((popped.unwrap().score - 0.7).abs() < 0.001);

        // Next pop should return 0.5
        let popped = BeamSearch::pop_fallback(&mut stack);
        assert!(popped.is_some());
        assert!((popped.unwrap().score - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_fallback_score_threshold() {
        let mut arena = Arena::new();
        let id0 = make_node_id(&mut arena);
        let id1 = make_node_id(&mut arena);
        let mut stack = Vec::new();

        // Score 0.01 with threshold 0.1 * 0.5 = 0.05 → should be rejected
        BeamSearch::push_fallback(
            &mut stack,
            FallbackEntry {
                path: SearchPath::from_node(id0, 0.01),
                score: 0.01,
            },
            0.1,
            0.5,
            100,
        );
        assert_eq!(stack.len(), 0, "Score below threshold should be rejected");

        // Score 0.06 with threshold 0.05 → should be accepted
        BeamSearch::push_fallback(
            &mut stack,
            FallbackEntry {
                path: SearchPath::from_node(id1, 0.06),
                score: 0.06,
            },
            0.1,
            0.5,
            100,
        );
        assert_eq!(stack.len(), 1, "Score above threshold should be accepted");
    }

    #[test]
    fn test_fallback_capacity_eviction() {
        let mut arena = Arena::new();
        let id0 = make_node_id(&mut arena);
        let id1 = make_node_id(&mut arena);
        let id2 = make_node_id(&mut arena);
        let mut stack = Vec::new();

        // Fill to capacity (max_size=2)
        BeamSearch::push_fallback(
            &mut stack,
            FallbackEntry {
                path: SearchPath::from_node(id0, 0.3),
                score: 0.3,
            },
            0.1,
            0.5,
            2,
        );
        BeamSearch::push_fallback(
            &mut stack,
            FallbackEntry {
                path: SearchPath::from_node(id1, 0.5),
                score: 0.5,
            },
            0.1,
            0.5,
            2,
        );
        assert_eq!(stack.len(), 2);

        // Push a higher-score entry → should evict the lowest (0.3)
        BeamSearch::push_fallback(
            &mut stack,
            FallbackEntry {
                path: SearchPath::from_node(id2, 0.8),
                score: 0.8,
            },
            0.1,
            0.5,
            2,
        );
        assert_eq!(stack.len(), 2);

        // Verify the 0.3 entry was evicted
        let scores: Vec<f32> = stack.iter().map(|e| e.score).collect();
        assert!(scores.contains(&0.5));
        assert!(scores.contains(&0.8));
        assert!(!scores.contains(&0.3));
    }

    #[test]
    fn test_fallback_empty_pop() {
        let mut stack: Vec<FallbackEntry> = Vec::new();
        assert!(BeamSearch::pop_fallback(&mut stack).is_none());
    }

    #[test]
    fn test_search_config_backtrack_defaults() {
        let config = SearchConfig::default();
        assert_eq!(config.max_backtracks, 3);
        assert!((config.fallback_score_ratio - 0.5).abs() < 0.001);
    }
}
