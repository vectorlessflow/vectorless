// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Orchestrator-level plan types.
//!
//! `OrchestratorPlan` is the Orchestrator's own plan: WHICH documents to query,
//! WHAT to ask each, and WITH what focus keywords.
//!
//! This is distinct from `QueryPlan` (about the query itself, from query understanding)
//! and `NavigationPlan` (about how to navigate one document's tree, built by the Worker).

// ---------------------------------------------------------------------------
// Dispatch target
// ---------------------------------------------------------------------------

/// A single dispatch target within an [`OrchestratorPlan`].
///
/// Created by the Orchestrator's analyze/replan phase, consumed by dispatch.
/// Each target produces one Worker.
#[derive(Debug, Clone)]
pub struct DispatchTarget {
    /// 0-based document index in the workspace.
    pub doc_idx: usize,
    /// LLM-generated reason for selecting this document.
    pub reason: String,
    /// Specific task/focus for the Worker to search for in this document.
    pub task: String,
    /// Focus keywords from ReasoningIndex to pass to the Worker.
    /// These are context for the Worker's LLM, not routing rules.
    pub focus_keywords: Vec<String>,
}

// ---------------------------------------------------------------------------
// Orchestrator plan
// ---------------------------------------------------------------------------

/// Orchestrator-level dispatch plan.
///
/// Describes WHICH documents to send Workers into and WHAT to ask each.
/// Produced by `analyze()` (initial plan) or `replan()` (subsequent round).
/// Consumed by the supervisor loop's dispatch phase.
#[derive(Debug, Clone)]
pub struct OrchestratorPlan {
    /// The dispatch targets for this round.
    pub targets: Vec<DispatchTarget>,
    /// LLM's reasoning about the plan (for logging/events).
    pub reasoning: String,
}

impl OrchestratorPlan {
    /// Create a plan that dispatches all documents (used when user specified docs).
    pub fn all_docs(doc_count: usize, query: &str) -> Self {
        Self {
            targets: (0..doc_count)
                .map(|idx| DispatchTarget {
                    doc_idx: idx,
                    reason: "User-specified document".to_string(),
                    task: query.to_string(),
                    focus_keywords: Vec::new(),
                })
                .collect(),
            reasoning: "User specified all documents".to_string(),
        }
    }

    /// Create an empty plan (no targets to dispatch).
    pub fn empty() -> Self {
        Self {
            targets: Vec::new(),
            reasoning: String::new(),
        }
    }

    /// Whether this plan has any targets to dispatch.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}
