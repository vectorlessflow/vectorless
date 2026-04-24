// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Content overlap map — marks overlapping regions to prevent duplicate visits.
//!
//! Built at compile time by comparing leaf node content pairwise with Jaccard
//! similarity. The Agent can skip nodes marked as overlapping, saving
//! navigation rounds.

use serde::{Deserialize, Serialize};

use super::node::NodeId;

/// Type of content overlap between two nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlapType {
    /// Content is nearly identical (Jaccard ≥ 0.9).
    Duplicate,
    /// One node's content is a subset of another's.
    Subset,
    /// One node is a summary of another.
    Summary,
}

/// A single overlap entry between two leaf nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlapEntry {
    /// First node.
    pub node_a: NodeId,
    /// Second node.
    pub node_b: NodeId,
    /// Jaccard similarity score (0.0–1.0).
    pub similarity: f64,
    /// Type of overlap detected.
    pub overlap_type: OverlapType,
}

/// Map of content overlaps across leaf nodes.
///
/// Built by the `OverlapPass` compiler pass. The Agent checks this map
/// when deciding whether to visit a node — if it's marked as overlapping
/// with an already-visited node, it can skip it.
///
/// # Construction
///
/// Pure compute — pairwise Jaccard on leaf node content. No LLM calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentOverlapMap {
    /// All detected overlaps.
    pub overlaps: Vec<OverlapEntry>,
}

impl ContentOverlapMap {
    /// Create a new empty overlap map.
    pub fn new() -> Self {
        Self { overlaps: Vec::new() }
    }

    /// Add an overlap entry.
    pub fn add(&mut self, entry: OverlapEntry) {
        self.overlaps.push(entry);
    }

    /// Check if a node has any overlaps with other nodes.
    pub fn has_overlap(&self, node_id: NodeId) -> bool {
        self.overlaps
            .iter()
            .any(|o| o.node_a == node_id || o.node_b == node_id)
    }

    /// Get all nodes that overlap with the given node.
    pub fn overlapping_nodes(&self, node_id: NodeId) -> Vec<(NodeId, f64, OverlapType)> {
        self.overlaps
            .iter()
            .filter_map(|o| {
                if o.node_a == node_id {
                    Some((o.node_b, o.similarity, o.overlap_type))
                } else if o.node_b == node_id {
                    Some((o.node_a, o.similarity, o.overlap_type))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Total number of overlap pairs.
    pub fn overlap_count(&self) -> usize {
        self.overlaps.len()
    }
}

impl Default for ContentOverlapMap {
    fn default() -> Self {
        Self::new()
    }
}
