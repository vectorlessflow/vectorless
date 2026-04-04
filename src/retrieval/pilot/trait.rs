// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pilot trait definition - the core interface for navigation intelligence.
//!
//! This module defines the [`Pilot`] trait which represents the brain of the
//! retrieval pipeline. Implementations provide navigation guidance at key
//! decision points during tree search.

use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::LazyLock;

use crate::domain::{DocumentTree, NodeId};

use super::{PilotConfig, PilotDecision, InterventionPoint};

/// Empty HashSet for use in SearchState::for_start
static EMPTY_VISITED: LazyLock<HashSet<NodeId>> = LazyLock::new(HashSet::new);

/// Search state passed to Pilot for decision making.
///
/// This struct contains all the context Pilot needs to understand
/// the current search situation and make informed decisions.
#[derive(Debug, Clone)]
pub struct SearchState<'a> {
    /// The document tree being searched.
    pub tree: &'a DocumentTree,
    /// The user's query string.
    pub query: &'a str,
    /// Current path from root to current node.
    pub path: &'a [NodeId],
    /// Candidate child nodes to evaluate.
    pub candidates: &'a [NodeId],
    /// Set of already visited nodes (to avoid cycles).
    pub visited: &'a HashSet<NodeId>,
    /// Current depth in the tree.
    pub depth: usize,
    /// Current search iteration number.
    pub iteration: usize,
    /// Best score found so far in this search.
    pub best_score: f32,
    /// Whether the search is currently backtracking.
    pub is_backtracking: bool,
}

impl<'a> SearchState<'a> {
    /// Create a new search state.
    pub fn new(
        tree: &'a DocumentTree,
        query: &'a str,
        path: &'a [NodeId],
        candidates: &'a [NodeId],
        visited: &'a HashSet<NodeId>,
    ) -> Self {
        Self {
            tree,
            query,
            path,
            candidates,
            visited,
            depth: path.len(),
            iteration: 0,
            best_score: 0.0,
            is_backtracking: false,
        }
    }

    /// Create a minimal search state for start guidance.
    pub fn for_start(tree: &'a DocumentTree, query: &'a str) -> Self {
        Self {
            tree,
            query,
            path: &[],
            candidates: &[],
            visited: &EMPTY_VISITED,
            depth: 0,
            iteration: 0,
            best_score: 0.0,
            is_backtracking: false,
        }
    }

    /// Check if we're at the root level.
    pub fn is_at_root(&self) -> bool {
        self.path.is_empty()
    }

    /// Check if there are multiple candidates (fork point).
    pub fn is_fork_point(&self) -> bool {
        self.candidates.len() > 1
    }

    /// Get the current node (last in path).
    pub fn current_node(&self) -> Option<NodeId> {
        self.path.last().copied()
    }
}

/// Pilot trait - the brain of the retrieval pipeline.
///
/// Pilot provides navigation guidance at key decision points during
/// tree search. It uses LLM intelligence for semantic understanding
/// while allowing the algorithm to handle efficient execution.
///
/// # Implementation Notes
///
/// Implementations should:
/// - Be cheap to construct
/// - Handle LLM failures gracefully
/// - Respect budget constraints
/// - Provide explainable decisions
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::pilot::{Pilot, SearchState, PilotDecision};
///
/// struct MyPilot;
///
/// #[async_trait]
/// impl Pilot for MyPilot {
///     fn name(&self) -> &str { "my_pilot" }
///
///     fn should_intervene(&self, state: &SearchState<'_>) -> bool {
///         state.candidates.len() > 3
///     }
///
///     async fn decide(&self, state: &SearchState<'_>) -> PilotDecision {
///         // LLM-based decision making
///         PilotDecision::default()
///     }
/// }
/// ```
#[async_trait]
pub trait Pilot: Send + Sync {
    /// Get the name of this Pilot implementation.
    fn name(&self) -> &str;

    /// Determine if Pilot should intervene at this point.
    ///
    /// This is the key method for controlling when LLM is called.
    /// Implementations should consider:
    /// - Candidate count (fork points)
    /// - Score uncertainty
    /// - Budget constraints
    /// - Current depth and iteration
    ///
    /// Returns `true` if Pilot should be consulted for a decision.
    fn should_intervene(&self, state: &SearchState<'_>) -> bool;

    /// Make a navigation decision.
    ///
    /// Called when `should_intervene` returns `true`.
    /// Implementations should:
    /// - Build appropriate context
    /// - Call LLM (if applicable)
    /// - Parse and validate response
    /// - Return a structured decision
    ///
    /// This method should never panic. On errors, return a default
    /// decision that preserves the original candidate order.
    async fn decide(&self, state: &SearchState<'_>) -> PilotDecision;

    /// Provide guidance before search starts.
    ///
    /// Called once at the beginning of search to help determine
    /// the starting point and initial direction.
    ///
    /// Returns `None` if no guidance is available or needed.
    async fn guide_start(
        &self,
        tree: &DocumentTree,
        query: &str,
    ) -> Option<PilotDecision>;

    /// Provide guidance during backtracking.
    ///
    /// Called when search needs to backtrack due to insufficient
    /// results. Pilot can analyze the failure and suggest
    /// alternative paths.
    ///
    /// Returns `None` if no guidance is available.
    async fn guide_backtrack(
        &self,
        state: &SearchState<'_>,
    ) -> Option<PilotDecision>;

    /// Get the current configuration.
    fn config(&self) -> &PilotConfig;

    /// Check if this Pilot is actually capable of providing guidance.
    ///
    /// Returns `false` for NoopPilot or when budget is exhausted.
    fn is_active(&self) -> bool {
        true
    }

    /// Reset internal state for a new query.
    ///
    /// Called at the start of each new search to reset
    /// budget counters, caches, and other per-query state.
    fn reset(&self);
}

/// Extension trait for Pilot with utility methods.
pub trait PilotExt: Pilot {
    /// Check if Pilot can intervene given current state and budget.
    fn can_intervene(&self, state: &SearchState<'_>) -> bool {
        self.is_active() && self.should_intervene(state)
    }

    /// Get the current intervention point type.
    fn intervention_point(&self, state: &SearchState<'_>) -> InterventionPoint {
        if state.is_at_root() || state.iteration == 0 {
            InterventionPoint::Start
        } else if state.is_backtracking {
            InterventionPoint::Backtrack
        } else if state.is_fork_point() {
            InterventionPoint::Fork
        } else {
            InterventionPoint::Evaluate
        }
    }
}

impl<T: Pilot + ?Sized> PilotExt for T {}
