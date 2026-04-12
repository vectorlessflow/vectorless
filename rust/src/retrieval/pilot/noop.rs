// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! NoopPilot - A no-operation Pilot implementation.
//!
//! This module provides a Pilot implementation that never intervenes,
//! useful for testing, benchmarking, and as a fallback when LLM
//! is unavailable.

use async_trait::async_trait;

use crate::document::{DocumentTree, NodeId};

use super::{InterventionPoint, Pilot, PilotConfig, PilotDecision, SearchState};

/// A Pilot implementation that never intervenes.
///
/// This is useful for:
/// - Testing the search algorithm without LLM interference
/// - Benchmarking baseline performance
/// - Fallback when LLM is unavailable
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::pilot::NoopPilot;
///
/// let pilot = NoopPilot::new();
///
/// // This will always return false
/// assert!(!pilot.should_intervene(&state));
/// ```
#[derive(Debug, Clone, Default)]
pub struct NoopPilot {
    config: PilotConfig,
}

impl NoopPilot {
    /// Create a new NoopPilot.
    pub fn new() -> Self {
        Self {
            config: PilotConfig::algorithm_only(),
        }
    }

    /// Create with custom config.
    pub fn with_config(config: PilotConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Pilot for NoopPilot {
    fn name(&self) -> &str {
        "noop"
    }

    fn should_intervene(&self, _state: &SearchState<'_>) -> bool {
        // Never intervene
        false
    }

    async fn decide(&self, state: &SearchState<'_>) -> PilotDecision {
        // Return a default decision that preserves original order
        let decision = PilotDecision::preserve_order(state.candidates);
        PilotDecision {
            intervention_point: InterventionPoint::Fork,
            ..decision
        }
    }

    async fn guide_start(
        &self,
        _tree: &DocumentTree,
        _query: &str,
        _start_node: NodeId,
    ) -> Option<PilotDecision> {
        // No guidance at start
        None
    }

    async fn guide_backtrack(&self, _state: &SearchState<'_>) -> Option<PilotDecision> {
        // No guidance during backtrack
        None
    }

    fn config(&self) -> &PilotConfig {
        &self.config
    }

    fn is_active(&self) -> bool {
        // NoopPilot is never active
        false
    }

    fn reset(&self) {
        // No state to reset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::NodeId;
    use std::collections::HashSet;

    #[test]
    fn test_noop_pilot_never_intervenes() {
        let pilot = NoopPilot::new();

        // Create a minimal state
        let tree = DocumentTree::new("test", "test content");
        let query = "test query";
        let path: &[NodeId] = &[];
        let candidates: &[NodeId] = &[];
        let visited = HashSet::new();

        let state = SearchState::new(&tree, query, path, candidates, &visited);

        // Should never intervene
        assert!(!pilot.should_intervene(&state));
    }

    #[tokio::test]
    async fn test_noop_pilot_returns_default_decision() {
        let pilot = NoopPilot::new();

        let tree = DocumentTree::new("test", "test content");
        let query = "test query";
        let path: &[NodeId] = &[];
        let candidates: &[NodeId] = &[];
        let visited = HashSet::new();

        let state = SearchState::new(&tree, query, path, candidates, &visited);
        let decision = pilot.decide(&state).await;

        assert_eq!(decision.confidence, 0.0);
        assert!(!decision.has_candidates());
    }

    #[tokio::test]
    async fn test_noop_pilot_no_start_guidance() {
        let pilot = NoopPilot::new();
        let tree = DocumentTree::new("test", "test content");

        let guidance = pilot.guide_start(&tree, "test", tree.root()).await;
        assert!(guidance.is_none());
    }

    #[test]
    fn test_noop_pilot_not_active() {
        let pilot = NoopPilot::new();
        assert!(!pilot.is_active());
    }
}
