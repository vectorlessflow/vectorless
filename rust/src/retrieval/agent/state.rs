// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Agent state types — mutable state that lives within a single retrieve() call.

use std::collections::HashSet;

use crate::document::NodeId;

use super::config::{Evidence, Output};

// ---------------------------------------------------------------------------
// SubAgent state
// ---------------------------------------------------------------------------

/// Mutable navigation state for a SubAgent loop.
///
/// Created at loop start, destroyed at loop end. Never escapes the call.
pub struct State {
    /// Navigation breadcrumb (path from root to current node).
    pub breadcrumb: Vec<String>,
    /// Current position in the document tree.
    pub current_node: NodeId,
    /// Collected evidence so far.
    pub evidence: Vec<Evidence>,
    /// Nodes already visited (prevents redundant reads).
    pub visited: HashSet<NodeId>,
    /// Remaining navigation rounds.
    pub remaining: u32,
    /// Maximum rounds (for display in prompts).
    pub max_rounds: u32,
    /// Feedback from the last executed command (injected into next prompt).
    pub last_feedback: String,
}

impl State {
    /// Create a new state starting at the given root node.
    pub fn new(root: NodeId, max_rounds: u32) -> Self {
        Self {
            breadcrumb: vec!["root".to_string()],
            current_node: root,
            evidence: Vec::new(),
            visited: HashSet::new(),
            remaining: max_rounds,
            max_rounds,
            last_feedback: String::new(),
        }
    }

    /// Consume the remaining rounds.
    pub fn dec_round(&mut self) {
        if self.remaining > 0 {
            self.remaining -= 1;
        }
    }

    /// Navigate into a child node.
    pub fn cd(&mut self, node: NodeId, title: &str) {
        self.breadcrumb.push(title.to_string());
        self.current_node = node;
        self.visited.insert(node);
    }

    /// Navigate back to parent.
    ///
    /// Returns `false` if already at root.
    pub fn cd_up(&mut self, parent: NodeId) -> bool {
        if self.breadcrumb.len() <= 1 {
            return false;
        }
        self.breadcrumb.pop();
        self.current_node = parent;
        true
    }

    /// Add a piece of evidence.
    pub fn add_evidence(&mut self, evidence: Evidence) {
        self.evidence.push(evidence);
    }

    /// Format the breadcrumb as a path string (e.g., "root/Chapter 1/Section 1.2").
    pub fn path_str(&self) -> String {
        self.breadcrumb.join("/")
    }

    /// Summary of collected evidence for prompts.
    pub fn evidence_summary(&self) -> String {
        if self.evidence.is_empty() {
            return "(none)".to_string();
        }
        self.evidence
            .iter()
            .map(|e| format!("- [{}] {} chars", e.node_title, e.content.len()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Convert this state into an Output (consuming the state).
    pub fn into_output(self, llm_calls: u32) -> Output {
        Output {
            answer: String::new(), // filled by synthesis
            evidence: self.evidence,
            metrics: super::config::Metrics {
                rounds_used: self.max_rounds.saturating_sub(self.remaining),
                llm_calls,
                nodes_visited: self.visited.len(),
                fast_path_hit: false,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Orchestrator state
// ---------------------------------------------------------------------------

/// Mutable state for the Orchestrator loop.
///
/// Tracks which documents have been dispatched and collects SubAgent results.
pub struct OrchestratorState {
    /// Indices of documents that have been dispatched.
    pub dispatched: Vec<usize>,
    /// Results returned by dispatched SubAgents.
    pub sub_results: Vec<Output>,
    /// All evidence merged from sub-results.
    pub all_evidence: Vec<Evidence>,
    /// Whether the analysis phase is complete.
    pub analyze_done: bool,
    /// Remaining integration retry count (max 1).
    pub integrate_retries: u32,
    /// Total LLM calls across orchestrator + sub-agents.
    pub total_llm_calls: u32,
}

impl OrchestratorState {
    /// Create a new orchestrator state.
    pub fn new() -> Self {
        Self {
            dispatched: Vec::new(),
            sub_results: Vec::new(),
            all_evidence: Vec::new(),
            analyze_done: false,
            integrate_retries: 1,
            total_llm_calls: 0,
        }
    }

    /// Record a dispatch to document at the given index.
    pub fn record_dispatch(&mut self, doc_idx: usize) {
        if !self.dispatched.contains(&doc_idx) {
            self.dispatched.push(doc_idx);
        }
    }

    /// Collect a SubAgent result.
    pub fn collect_result(&mut self, result: Output) {
        self.total_llm_calls += result.metrics.llm_calls;
        self.all_evidence.extend(result.evidence.iter().cloned());
        self.sub_results.push(result);
    }

    /// Merge all sub-results into a single Output.
    pub fn into_output(self, answer: String) -> Output {
        Output {
            answer,
            evidence: self.all_evidence,
            metrics: super::config::Metrics {
                rounds_used: 0,
                llm_calls: self.total_llm_calls,
                nodes_visited: self
                    .sub_results
                    .iter()
                    .map(|r| r.metrics.nodes_visited)
                    .sum(),
                fast_path_hit: false,
            },
        }
    }
}

impl Default for OrchestratorState {
    fn default() -> Self {
        Self::new()
    }
}
