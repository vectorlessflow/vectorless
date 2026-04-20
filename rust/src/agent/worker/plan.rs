// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Navigation plan — strategy-driven guidance for the Worker's navigation loop.

use crate::document::NodeId;

/// Navigation strategy selected by the planning phase.
#[derive(Debug, Clone)]
pub enum NavStrategy {
    /// ReasoningIndex high-confidence hit — navigate directly and collect.
    DirectHit { targets: Vec<TargetNode> },
    /// Broad scan — read summaries to get an overview.
    SummaryScan,
    /// Section map provides direct access — jump to known section.
    StructuredNav { section: String },
    /// Full ReAct loop — LLM-driven exploration with no clear starting point.
    DeepNavigation,
}

impl Default for NavStrategy {
    fn default() -> Self {
        Self::DeepNavigation
    }
}

/// A high-confidence target node from the planning phase.
#[derive(Debug, Clone)]
pub struct TargetNode {
    pub node_id: NodeId,
    pub confidence: f32,
}

/// A hint from keyword matching to guide navigation.
#[derive(Debug, Clone)]
pub struct RouteHint {
    pub keyword: String,
    pub node_id: NodeId,
    pub node_title: String,
    pub weight: f32,
}

/// A structured navigation plan produced by the Worker's planning phase.
///
/// Replaces the previous `state.plan: String` with structured data that
/// the navigation loop can use to choose strategy-specific behavior.
#[derive(Debug, Clone)]
pub struct NavigationPlan {
    pub strategy: NavStrategy,
    /// Entry node for navigation (if known from fast-path misses).
    pub entry_node: Option<NodeId>,
    /// Keywords and their matching nodes.
    pub route_hints: Vec<RouteHint>,
}

impl Default for NavigationPlan {
    fn default() -> Self {
        Self {
            strategy: NavStrategy::DeepNavigation,
            entry_node: None,
            route_hints: Vec::new(),
        }
    }
}
