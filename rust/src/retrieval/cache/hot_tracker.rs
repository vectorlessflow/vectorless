// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Hot node tracker for recording retrieval frequency.
//!
//! Thread-safe tracker that records which nodes are frequently retrieved.
//! Nodes that exceed a configured hit-count threshold are marked as "hot",
//! which can boost their scores in future retrieval operations.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::document::NodeId;
use crate::document::HotNodeEntry;

/// Thread-safe tracker for hot (frequently retrieved) nodes.
pub struct HotNodeTracker {
    inner: RwLock<HotNodeTrackerInner>,
    hot_threshold: u32,
}

struct HotNodeTrackerInner {
    hits: HashMap<NodeId, u32>,
    scores: HashMap<NodeId, f32>,
}

impl HotNodeTracker {
    /// Create a new tracker with the given hot threshold.
    pub fn new(hot_threshold: u32) -> Self {
        Self {
            inner: RwLock::new(HotNodeTrackerInner {
                hits: HashMap::new(),
                scores: HashMap::new(),
            }),
            hot_threshold,
        }
    }

    /// Record that a node was retrieved with a given score.
    pub fn record_hit(&self, node_id: NodeId, score: f32) {
        if let Ok(mut inner) = self.inner.write() {
            let hits = *inner.hits.entry(node_id).or_insert(0) + 1;
            inner.hits.insert(node_id, hits);

            // Update running average score
            let prev_avg = *inner.scores.entry(node_id).or_insert(0.0);
            let new_avg = prev_avg + (score - prev_avg) / hits as f32;
            inner.scores.insert(node_id, new_avg);
        }
    }

    /// Record multiple hits at once.
    pub fn record_hits(&self, hits: &[(NodeId, f32)]) {
        for &(node_id, score) in hits {
            self.record_hit(node_id, score);
        }
    }

    /// Check if a node is considered "hot".
    pub fn is_hot(&self, node_id: NodeId) -> bool {
        self.inner
            .read()
            .map(|inner| {
                inner.hits.get(&node_id).copied().unwrap_or(0) >= self.hot_threshold
            })
            .unwrap_or(false)
    }

    /// Get the hit count for a node.
    pub fn hit_count(&self, node_id: NodeId) -> u32 {
        self.inner
            .read()
            .map(|inner| inner.hits.get(&node_id).copied().unwrap_or(0))
            .unwrap_or(0)
    }

    /// Get all hot nodes with their stats.
    pub fn hot_nodes(&self) -> Vec<(NodeId, u32, f32)> {
        self.inner
            .read()
            .map(|inner| {
                inner
                    .hits
                    .iter()
                    .filter(|(_, count)| **count >= self.hot_threshold)
                    .map(|(node_id, count)| {
                        (
                            *node_id,
                            *count,
                            inner.scores.get(node_id).copied().unwrap_or(0.0),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Export hot node data into HotNodeEntry map for persistence.
    pub fn export(&self) -> HashMap<NodeId, HotNodeEntry> {
        self.inner
            .read()
            .map(|inner| {
                inner
                    .hits
                    .iter()
                    .map(|(node_id, hit_count)| {
                        let avg_score = inner.scores.get(node_id).copied().unwrap_or(0.0);
                        let is_hot = *hit_count >= self.hot_threshold;
                        (
                            *node_id,
                            HotNodeEntry {
                                hit_count: *hit_count,
                                avg_score,
                                is_hot,
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the hot threshold.
    pub fn hot_threshold(&self) -> u32 {
        self.hot_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node_ids() -> (NodeId, NodeId, NodeId) {
        let mut tree = crate::document::DocumentTree::new("Root", "content");
        let a = tree.add_child(tree.root(), "A", "a");
        let b = tree.add_child(tree.root(), "B", "b");
        let c = tree.add_child(tree.root(), "C", "c");
        (a, b, c)
    }

    #[test]
    fn test_hot_tracker_basic() {
        let tracker = HotNodeTracker::new(3);

        let (node, _, _) = make_node_ids();
        tracker.record_hit(node, 0.8);
        tracker.record_hit(node, 0.9);
        assert!(!tracker.is_hot(node));
        assert_eq!(tracker.hit_count(node), 2);

        tracker.record_hit(node, 0.7);
        assert!(tracker.is_hot(node));
        assert_eq!(tracker.hit_count(node), 3);
    }

    #[test]
    fn test_hot_tracker_export() {
        let tracker = HotNodeTracker::new(2);

        let (node_a, node_b, _) = make_node_ids();

        tracker.record_hit(node_a, 0.8);
        tracker.record_hit(node_a, 0.9);
        tracker.record_hit(node_b, 0.5);

        let exported = tracker.export();
        assert!(exported[&node_a].is_hot);
        assert!(!exported[&node_b].is_hot);
    }

    #[test]
    fn test_hot_tracker_multiple_hits() {
        let tracker = HotNodeTracker::new(1);

        let (node_a, node_b, node_c) = make_node_ids();

        let hits = vec![
            (node_a, 0.9),
            (node_b, 0.8),
            (node_c, 0.7),
        ];
        tracker.record_hits(&hits);

        assert!(tracker.is_hot(node_a));
        assert!(tracker.is_hot(node_b));
        assert!(tracker.is_hot(node_c));

        let hot = tracker.hot_nodes();
        assert_eq!(hot.len(), 3);
    }
}
