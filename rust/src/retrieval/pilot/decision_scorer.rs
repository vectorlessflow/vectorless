// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Shared Pilot-as-primary scoring helper.
//!
//! All three search algorithms (PurePilot, Beam, MCTS) use this module
//! to score child candidates. Pilot is the primary scorer; NodeScorer
//! provides a fallback when Pilot is unavailable or budget is exhausted.
//!
//! # Caching
//!
//! Pilot decisions are cached by `(query, parent_node_id)` to avoid
//! redundant LLM calls when the same node is revisited (e.g. MCTS
//! selection phase revisits a node multiple times).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::scorer::{NodeScorer, ScoringContext};
use crate::document::{DocumentTree, NodeId};
use crate::retrieval::pilot::{Pilot, PilotDecision, SearchState};

/// Cache key: (query_fingerprint, parent_node_id).
type CacheKey = (u64, NodeId);

/// Shared Pilot decision cache.
///
/// Thread-safe, query-scoped cache that stores Pilot decisions keyed by
/// (query hash, parent node ID). Prevents redundant LLM calls when the
/// same (query, node) pair is scored multiple times (common in MCTS).
#[derive(Debug, Clone, Default)]
pub struct PilotDecisionCache {
    inner: Arc<Mutex<HashMap<CacheKey, PilotDecision>>>,
}

impl PilotDecisionCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute cache key from query and parent node.
    fn cache_key(query: &str, parent: NodeId) -> CacheKey {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        query.hash(&mut hasher);
        (hasher.finish(), parent)
    }

    /// Try to get a cached decision.
    pub async fn get(&self, query: &str, parent: NodeId) -> Option<PilotDecision> {
        let key = Self::cache_key(query, parent);
        let cache = self.inner.lock().await;
        cache.get(&key).cloned()
    }

    /// Store a decision in the cache.
    pub async fn put(&self, query: &str, parent: NodeId, decision: &PilotDecision) {
        let key = Self::cache_key(query, parent);
        let mut cache = self.inner.lock().await;
        cache.entry(key).or_insert_with(|| decision.clone());
    }

    /// Clear the cache.
    pub async fn clear(&self) {
        self.inner.lock().await.clear();
    }
}

/// Score child candidates using Pilot as primary, NodeScorer as fallback.
///
/// Pilot decisions are cached by (query, parent_node_id). Subsequent calls
/// with the same arguments return cached results without LLM calls.
///
/// `pilot_weight` controls how much Pilot vs NodeScorer contributes:
/// - 1.0 = PurePilot (only Pilot scores matter)
/// - 0.7 = Beam (Pilot dominant, NodeScorer as secondary)
/// - 0.5 = MCTS prior (balanced)
pub async fn score_candidates(
    tree: &DocumentTree,
    candidates: &[NodeId],
    query: &str,
    pilot: Option<&dyn Pilot>,
    path: &[NodeId],
    visited: &HashSet<NodeId>,
    pilot_weight: f32,
    cache: Option<&PilotDecisionCache>,
    step_reasons: Option<&[Option<String>]>,
) -> Vec<(NodeId, f32)> {
    let scored = score_candidates_detailed(
        tree, candidates, query, pilot, path, visited, pilot_weight, cache, step_reasons,
    )
    .await;
    scored.into_iter().map(|s| (s.node_id, s.score)).collect()
}

/// A scored candidate with optional reasoning from the Pilot.
#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    /// The node ID.
    pub node_id: NodeId,
    /// Relevance score (0.0 - 1.0).
    pub score: f32,
    /// Reason the Pilot chose this node, if available.
    pub reason: Option<String>,
}

/// Score child candidates and return detailed results with reasons.
///
/// Like [`score_candidates`] but preserves per-candidate reasoning
/// from the Pilot. Use this when the search algorithm needs to
/// record why each path step was taken (e.g., for beam search
/// reasoning history).
pub async fn score_candidates_detailed(
    tree: &DocumentTree,
    candidates: &[NodeId],
    query: &str,
    pilot: Option<&dyn Pilot>,
    path: &[NodeId],
    visited: &HashSet<NodeId>,
    pilot_weight: f32,
    cache: Option<&PilotDecisionCache>,
    step_reasons: Option<&[Option<String>]>,
) -> Vec<ScoredCandidate> {
    if candidates.is_empty() {
        return Vec::new();
    }

    // If no Pilot, pure NodeScorer (no reasons available)
    let Some(p) = pilot else {
        return score_with_scorer_detailed(tree, candidates, query);
    };

    if !p.is_active() {
        return score_with_scorer_detailed(tree, candidates, query);
    }

    // Determine parent node (last in path) for cache key
    let parent = path.last().copied().unwrap_or(tree.root());

    // Check cache first
    let decision = if let Some(c) = cache {
        if let Some(cached) = c.get(query, parent).await {
            tracing::trace!("Pilot cache hit for parent={:?}", parent);
            cached
        } else {
            let mut state = SearchState::new(tree, query, path, candidates, visited);
            state.step_reasons = step_reasons;
            let d = p.decide(&state).await;
            c.put(query, parent, &d).await;
            d
        }
    } else {
        let mut state = SearchState::new(tree, query, path, candidates, visited);
        state.step_reasons = step_reasons;
        p.decide(&state).await
    };

    // Build Pilot score + reason map
    let mut pilot_data: HashMap<NodeId, (f32, Option<String>)> = HashMap::new();
    for ranked in &decision.ranked_candidates {
        pilot_data.insert(ranked.node_id, (ranked.score, ranked.reason.clone()));
    }

    // Compute NodeScorer fallback scores
    let scorer_weight = 1.0 - pilot_weight;
    let confidence = decision.confidence;
    let effective_pilot = pilot_weight * confidence;

    let scorer = NodeScorer::new(ScoringContext::new(query));

    let mut scored: Vec<ScoredCandidate> = candidates
        .iter()
        .map(|&node_id| {
            let algo_score = scorer.score(tree, node_id);
            let (p_score, reason) = pilot_data.get(&node_id)
                .map(|(s, r)| (*s, r.clone()))
                .unwrap_or((0.0, None));

            let final_score = if effective_pilot > 0.0 && pilot_data.contains_key(&node_id) {
                (effective_pilot * p_score + scorer_weight * algo_score)
                    / (effective_pilot + scorer_weight)
            } else {
                algo_score
            };

            ScoredCandidate { node_id, score: final_score, reason }
        })
        .collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

/// Pure NodeScorer fallback.
fn score_with_scorer(
    tree: &DocumentTree,
    candidates: &[NodeId],
    query: &str,
) -> Vec<(NodeId, f32)> {
    let scorer = NodeScorer::new(ScoringContext::new(query));
    scorer.score_and_sort(tree, candidates)
}

/// Pure NodeScorer fallback returning detailed results (no reasons).
fn score_with_scorer_detailed(
    tree: &DocumentTree,
    candidates: &[NodeId],
    query: &str,
) -> Vec<ScoredCandidate> {
    let scorer = NodeScorer::new(ScoringContext::new(query));
    scorer
        .score_and_sort(tree, candidates)
        .into_iter()
        .map(|(node_id, score)| ScoredCandidate { node_id, score, reason: None })
        .collect()
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
    fn test_cache_key_deterministic() {
        let mut arena = Arena::new();
        let nid = make_node_id(&mut arena);

        let key1 = PilotDecisionCache::cache_key("hello", nid);
        let key2 = PilotDecisionCache::cache_key("hello", nid);
        assert_eq!(key1, key2);

        let key3 = PilotDecisionCache::cache_key("world", nid);
        assert_ne!(key1, key3);
    }
}
