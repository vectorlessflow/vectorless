// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Decision types for Pilot navigation.
//!
//! This module defines the types that represent Pilot's navigation decisions,
//! including direction recommendations, candidate rankings, and intervention points.

use serde::{Deserialize, Serialize};

use crate::document::NodeId;

/// Pilot's navigation decision result.
///
/// Contains all information about where to go next and why.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PilotDecision {
    /// Ranked list of candidate nodes (most relevant first).
    pub ranked_candidates: Vec<RankedCandidate>,
    /// Recommended search direction.
    pub direction: SearchDirection,
    /// Confidence level of this decision (0.0 - 1.0).
    pub confidence: f32,
    /// Human-readable explanation of the decision.
    pub reasoning: String,
    /// The intervention point that triggered this decision.
    pub intervention_point: InterventionPoint,
}

impl Default for PilotDecision {
    fn default() -> Self {
        Self {
            ranked_candidates: Vec::new(),
            direction: SearchDirection::GoDeeper {
                reason: "Default decision".to_string(),
            },
            confidence: 0.0,
            reasoning: "No specific guidance available".to_string(),
            intervention_point: InterventionPoint::Evaluate,
        }
    }
}

impl PilotDecision {
    /// Create a new decision with the given candidates and direction.
    pub fn new(
        ranked_candidates: Vec<RankedCandidate>,
        direction: SearchDirection,
        confidence: f32,
        reasoning: String,
    ) -> Self {
        Self {
            ranked_candidates,
            direction,
            confidence,
            reasoning,
            intervention_point: InterventionPoint::Fork,
        }
    }

    /// Create a decision that preserves original order (no-op).
    pub fn preserve_order(candidates: &[NodeId]) -> Self {
        Self {
            ranked_candidates: candidates
                .iter()
                .enumerate()
                .map(|(i, &id)| RankedCandidate {
                    node_id: id,
                    score: 1.0 - (i as f32 * 0.1),
                    reason: None,
                })
                .collect(),
            direction: SearchDirection::GoDeeper {
                reason: "Preserving original order".to_string(),
            },
            confidence: 0.0,
            reasoning: "No intervention performed".to_string(),
            intervention_point: InterventionPoint::Fork,
        }
    }

    /// Check if this decision has any ranked candidates.
    pub fn has_candidates(&self) -> bool {
        !self.ranked_candidates.is_empty()
    }

    /// Get the top-ranked candidate.
    pub fn top_candidate(&self) -> Option<&RankedCandidate> {
        self.ranked_candidates.first()
    }

    /// Get node IDs in ranked order.
    pub fn ranked_node_ids(&self) -> Vec<NodeId> {
        self.ranked_candidates.iter().map(|c| c.node_id).collect()
    }

    /// Check if the decision indicates an answer was found.
    pub fn found_answer(&self) -> bool {
        matches!(self.direction, SearchDirection::FoundAnswer { .. })
    }

    /// Check if the decision indicates backtracking is needed.
    pub fn needs_backtrack(&self) -> bool {
        matches!(self.direction, SearchDirection::Backtrack { .. })
    }
}

/// A ranked candidate node with score and optional reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedCandidate {
    /// The node ID.
    pub node_id: NodeId,
    /// Relevance score (0.0 - 1.0).
    pub score: f32,
    /// Optional reason for this ranking.
    pub reason: Option<String>,
}

impl RankedCandidate {
    /// Create a new ranked candidate.
    pub fn new(node_id: NodeId, score: f32) -> Self {
        Self {
            node_id,
            score,
            reason: None,
        }
    }

    /// Create with a reason.
    pub fn with_reason(node_id: NodeId, score: f32, reason: impl Into<String>) -> Self {
        Self {
            node_id,
            score,
            reason: Some(reason.into()),
        }
    }

    /// Set the reason for this ranking.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// Search direction recommendation from Pilot.
///
/// Indicates where the search should go next.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchDirection {
    /// Continue deeper into the current branch.
    GoDeeper {
        /// Reason for going deeper.
        reason: String,
    },
    /// Explore sibling nodes at the same level.
    ExploreSiblings {
        /// Recommended siblings to explore.
        recommended: Vec<NodeId>,
    },
    /// Backtrack to parent and try other branches.
    Backtrack {
        /// Reason for backtracking.
        reason: String,
        /// Alternative branches to try.
        alternative_branches: Vec<NodeId>,
    },
    /// Jump to a non-local node (global navigation).
    JumpTo {
        /// Target node to jump to.
        target: NodeId,
        /// Reason for the jump.
        reason: String,
    },
    /// Current node contains the answer.
    FoundAnswer {
        /// Confidence that this is the answer.
        confidence: f32,
    },
}

impl SearchDirection {
    /// Create a GoDeeper direction.
    pub fn go_deeper(reason: impl Into<String>) -> Self {
        Self::GoDeeper {
            reason: reason.into(),
        }
    }

    /// Create a Backtrack direction.
    pub fn backtrack(reason: impl Into<String>, alternatives: Vec<NodeId>) -> Self {
        Self::Backtrack {
            reason: reason.into(),
            alternative_branches: alternatives,
        }
    }

    /// Create a JumpTo direction.
    pub fn jump_to(target: NodeId, reason: impl Into<String>) -> Self {
        Self::JumpTo {
            target,
            reason: reason.into(),
        }
    }

    /// Create a FoundAnswer direction.
    pub fn found_answer(confidence: f32) -> Self {
        Self::FoundAnswer { confidence }
    }
}

/// The point in search where Pilot intervenes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum InterventionPoint {
    /// Before search begins (initial guidance).
    Start,
    /// At a fork with multiple candidates.
    #[default]
    Fork,
    /// During backtracking after a dead end.
    Backtrack,
    /// Evaluating a specific node for relevance.
    Evaluate,
}

impl InterventionPoint {
    /// Get a human-readable name for this point.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Fork => "fork",
            Self::Backtrack => "backtrack",
            Self::Evaluate => "evaluate",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indextree::Arena;

    fn create_test_node_ids(count: usize) -> Vec<NodeId> {
        let mut arena = Arena::new();
        let mut ids = Vec::new();
        for i in 0..count {
            let node = crate::document::TreeNode {
                title: format!("Node {}", i),
                structure: String::new(),
                content: String::new(),
                summary: String::new(),
                depth: 0,
                start_index: 1,
                end_index: 1,
                start_page: None,
                end_page: None,
                node_id: None,
                physical_index: None,
                token_count: None,
                references: Vec::new(),
            };
            ids.push(NodeId(arena.new_node(node)));
        }
        ids
    }

    #[test]
    fn test_pilot_decision_default() {
        let decision = PilotDecision::default();
        assert!(!decision.has_candidates());
        assert!(decision.top_candidate().is_none());
        assert!(!decision.found_answer());
        assert!(!decision.needs_backtrack());
    }

    #[test]
    fn test_pilot_decision_preserve_order() {
        let node_ids = create_test_node_ids(2);
        let decision = PilotDecision::preserve_order(&node_ids);

        assert!(decision.has_candidates());
        assert_eq!(decision.ranked_candidates.len(), 2);
        assert_eq!(decision.confidence, 0.0);
    }

    #[test]
    fn test_ranked_candidate() {
        let node_ids = create_test_node_ids(1);
        let candidate = RankedCandidate::new(node_ids[0], 0.8);
        assert_eq!(candidate.score, 0.8);
        assert!(candidate.reason.is_none());

        let candidate_with_reason = RankedCandidate::with_reason(node_ids[0], 0.9, "test reason");
        assert_eq!(candidate_with_reason.score, 0.9);
        assert_eq!(
            candidate_with_reason.reason,
            Some("test reason".to_string())
        );
    }

    #[test]
    fn test_search_direction_constructors() {
        let deeper = SearchDirection::go_deeper("test");
        assert!(matches!(deeper, SearchDirection::GoDeeper { .. }));

        let found = SearchDirection::found_answer(0.9);
        assert!(matches!(
            found,
            SearchDirection::FoundAnswer { confidence: 0.9 }
        ));
    }

    #[test]
    fn test_intervention_point() {
        assert_eq!(InterventionPoint::Start.name(), "start");
        assert_eq!(InterventionPoint::Fork.name(), "fork");
        assert_eq!(InterventionPoint::Backtrack.name(), "backtrack");
        assert_eq!(InterventionPoint::Evaluate.name(), "evaluate");
    }
}
