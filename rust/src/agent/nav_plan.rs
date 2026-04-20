// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Navigation plan — strategy-driven guidance for the Worker's navigation loop.
//!
//! This is the Worker's own plan type: HOW to navigate one document's tree.
//! Distinct from `OrchestratorPlan` (which docs to query) and `QueryPlan` (query analysis).
//!
//! Strategy is determined by LLM reasoning, not by keyword thresholds.
//! ReasoningIndex hits are passed as context to the LLM, not as routing rules.

use crate::document::NodeId;

/// Navigation strategy selected by the planning phase.
#[derive(Debug, Clone)]
pub enum NavStrategy {
    /// High-confidence targets identified by LLM from index signals — collect directly.
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
/// Presented to the LLM as context, not used as a routing rule.
#[derive(Debug, Clone)]
pub struct RouteHint {
    pub keyword: String,
    pub node_id: NodeId,
    pub node_title: String,
    pub weight: f32,
}

/// A structured navigation plan produced by the Worker's planning phase.
///
/// The Worker builds this via LLM reasoning. Index signals (keyword hits,
/// section map entries) are provided as context to the LLM, which decides
/// the appropriate strategy.
#[derive(Debug, Clone)]
pub struct NavigationPlan {
    pub strategy: NavStrategy,
    /// Entry node for navigation (if known from index signals).
    pub entry_node: Option<NodeId>,
    /// Keywords and their matching nodes — context for the LLM.
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
