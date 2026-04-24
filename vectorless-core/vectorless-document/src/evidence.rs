// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Evidence score map — per-node quality metrics computed at compile time.
//!
//! Helps the Agent prioritize high-value nodes and skip low-information content.
//! All metrics are computed from content analysis, no LLM calls required.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::node::NodeId;

/// Per-node evidence quality scores.
///
/// Each metric ranges from 0.0 to 1.0. Higher is better.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceScore {
    /// Information density: ratio of unique meaningful tokens to total tokens.
    /// High density = content is packed with facts rather than filler.
    pub density: f64,
    /// Data richness: presence of numbers, tables, code blocks, lists.
    /// High richness = content contains structured data.
    pub data_richness: f64,
    /// Specificity: how narrowly focused the content is on a specific topic.
    /// High specificity = content is targeted, not generic prose.
    pub specificity: f64,
}

impl EvidenceScore {
    /// Composite score (weighted average of all metrics).
    pub fn composite(&self) -> f64 {
        self.density * 0.4 + self.data_richness * 0.3 + self.specificity * 0.3
    }
}

/// Map of evidence scores for all leaf nodes.
///
/// Built by the `ScorePass` compiler pass. The Agent can use these scores
/// to decide which nodes to visit first when multiple candidates exist.
///
/// # Construction
///
/// Pure compute — statistical analysis of node content. No LLM calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceScoreMap {
    /// Scores for each scored node (typically leaf nodes).
    #[serde(with = "super::serde_helpers")]
    scores: HashMap<NodeId, EvidenceScore>,
}

impl EvidenceScoreMap {
    /// Create a new empty score map.
    pub fn new() -> Self {
        Self {
            scores: HashMap::new(),
        }
    }

    /// Add a score for a node.
    pub fn insert(&mut self, node_id: NodeId, score: EvidenceScore) {
        self.scores.insert(node_id, score);
    }

    /// Get the score for a specific node.
    pub fn get(&self, node_id: NodeId) -> Option<&EvidenceScore> {
        self.scores.get(&node_id)
    }

    /// Get the composite score for a node, defaulting to 0.0.
    pub fn composite_for(&self, node_id: NodeId) -> f64 {
        self.scores.get(&node_id).map(|s| s.composite()).unwrap_or(0.0)
    }

    /// Get nodes sorted by composite score (highest first).
    pub fn ranked_nodes(&self) -> Vec<(NodeId, f64)> {
        let mut nodes: Vec<_> = self
            .scores
            .iter()
            .map(|(id, s)| (*id, s.composite()))
            .collect();
        nodes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        nodes
    }

    /// Number of scored nodes.
    pub fn len(&self) -> usize {
        self.scores.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
    }

    /// Access the underlying scores map.
    pub fn scores(&self) -> &HashMap<NodeId, EvidenceScore> {
        &self.scores
    }
}

impl Default for EvidenceScoreMap {
    fn default() -> Self {
        Self::new()
    }
}
