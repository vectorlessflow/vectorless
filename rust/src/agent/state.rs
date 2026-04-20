// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Agent state types — mutable state that lives within a single retrieve() call.

use std::collections::HashSet;

use crate::document::NodeId;

use super::config::{Evidence, Output};

// ---------------------------------------------------------------------------
// Worker state
// ---------------------------------------------------------------------------

/// Mutable navigation state for a Worker loop.
///
/// Created at loop start, destroyed at loop end. Never escapes the call.
pub struct WorkerState {
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
    /// Structured description of what information is still missing.
    /// Updated after `check` returns "insufficient".
    pub missing_info: String,
    /// ReAct history: summary of each round's command + result.
    /// Keeps last N entries for prompt injection.
    pub history: Vec<String>,
    /// Navigation plan generated after bird's-eye view (Phase 1.5).
    /// Injected into subsequent prompts as guidance (non-binding).
    pub plan: String,
    /// Number of times `check` has been called.
    pub check_count: u32,
    /// Whether a navigation plan was generated in Phase 1.5.
    pub plan_generated: bool,
}

/// Maximum number of history entries to keep for prompt injection.
const MAX_HISTORY_ENTRIES: usize = 6;

/// Maximum characters for `last_feedback` before truncation.
/// Prevents large cat/grep outputs from bloating subsequent prompts.
const MAX_FEEDBACK_CHARS: usize = 500;

impl WorkerState {
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
            missing_info: String::new(),
            history: Vec::new(),
            plan: String::new(),
            check_count: 0,
            plan_generated: false,
        }
    }

    /// Consume the remaining rounds.
    pub fn dec_round(&mut self) {
        if self.remaining > 0 {
            self.remaining -= 1;
        }
    }

    /// Set feedback with automatic truncation to prevent prompt bloat.
    pub fn set_feedback(&mut self, feedback: String) {
        if feedback.len() <= MAX_FEEDBACK_CHARS {
            self.last_feedback = feedback;
        } else {
            // Find a clean truncation point (line boundary if possible)
            let truncated = &feedback[..MAX_FEEDBACK_CHARS];
            let end = truncated.rfind('\n').unwrap_or(MAX_FEEDBACK_CHARS);
            self.last_feedback = format!(
                "{}...\n(truncated, {} chars total)",
                &feedback[..end.min(MAX_FEEDBACK_CHARS)],
                feedback.len()
            );
        }
    }

    /// Navigate into a child node.
    pub fn cd(&mut self, node: NodeId, title: &str) {
        self.breadcrumb.push(title.to_string());
        self.current_node = node;
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

    /// Push a history entry (command + result summary).
    /// Keeps only the last `MAX_HISTORY_ENTRIES` entries.
    pub fn push_history(&mut self, entry: String) {
        if self.history.len() >= MAX_HISTORY_ENTRIES {
            self.history.remove(0);
        }
        self.history.push(entry);
    }

    /// Format history as text for prompt injection.
    pub fn history_text(&self) -> String {
        if self.history.is_empty() {
            return "(no history yet)".to_string();
        }
        self.history
            .iter()
            .enumerate()
            .map(|(i, h)| format!("{}. {}", i + 1, h))
            .collect::<Vec<_>>()
            .join("\n")
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

    /// Convert this state into a WorkerOutput (consuming the state), with budget flag.
    /// Worker returns evidence only — no answer synthesis.
    pub fn into_worker_output(
        self,
        llm_calls: u32,
        budget_exhausted: bool,
        doc_name: &str,
    ) -> super::config::WorkerOutput {
        let evidence_chars: usize = self.evidence.iter().map(|e| e.content.len()).sum();
        super::config::WorkerOutput {
            evidence: self.evidence,
            metrics: super::config::WorkerMetrics {
                rounds_used: self.max_rounds.saturating_sub(self.remaining),
                llm_calls,
                nodes_visited: self.visited.len(),
                budget_exhausted,
                plan_generated: self.plan_generated,
                check_count: self.check_count,
                evidence_chars,
            },
            doc_name: doc_name.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Orchestrator state
// ---------------------------------------------------------------------------

/// Mutable state for the Orchestrator loop.
///
/// Tracks which documents have been dispatched and collects Worker results.
pub struct OrchestratorState {
    /// Indices of documents that have been dispatched.
    pub dispatched: Vec<usize>,
    /// Results returned by dispatched Workers.
    pub sub_results: Vec<Output>,
    /// All evidence merged from sub-results.
    pub all_evidence: Vec<Evidence>,
    /// Whether the analysis phase is complete.
    pub analyze_done: bool,
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
            total_llm_calls: 0,
        }
    }

    /// Record a dispatch to document at the given index.
    pub fn record_dispatch(&mut self, doc_idx: usize) {
        if !self.dispatched.contains(&doc_idx) {
            self.dispatched.push(doc_idx);
        }
    }

    /// Collect a Worker result, converting WorkerOutput to Output for internal tracking.
    pub fn collect_result(&mut self, doc_idx: usize, result: super::config::WorkerOutput) {
        self.total_llm_calls += result.metrics.llm_calls;
        self.all_evidence.extend(result.evidence.iter().cloned());
        self.sub_results.push(result.into());
        self.record_dispatch(doc_idx);
    }

    /// Clone results into an Output without consuming self.
    ///
    /// Used by `finalize_output` which needs to borrow state for rerank.
    pub fn clone_results_into_output(&self, answer: String) -> Output {
        Output {
            answer,
            evidence: self.all_evidence.clone(),
            metrics: super::config::Metrics {
                llm_calls: self.total_llm_calls,
                nodes_visited: self
                    .sub_results
                    .iter()
                    .map(|r| r.metrics.nodes_visited)
                    .sum(),
                plan_generated: self.sub_results.iter().any(|r| r.metrics.plan_generated),
                check_count: self.sub_results.iter().map(|r| r.metrics.check_count).sum(),
                evidence_chars: self
                    .sub_results
                    .iter()
                    .map(|r| r.metrics.evidence_chars)
                    .sum(),
                ..Default::default()
            },
            confidence: 0.0,
        }
    }

    /// Merge all sub-results into a single Output (consuming self).
    pub fn into_output(self, answer: String) -> Output {
        Output {
            answer,
            evidence: self.all_evidence,
            metrics: super::config::Metrics {
                llm_calls: self.total_llm_calls,
                nodes_visited: self
                    .sub_results
                    .iter()
                    .map(|r| r.metrics.nodes_visited)
                    .sum(),
                plan_generated: self.sub_results.iter().any(|r| r.metrics.plan_generated),
                check_count: self.sub_results.iter().map(|r| r.metrics.check_count).sum(),
                evidence_chars: self
                    .sub_results
                    .iter()
                    .map(|r| r.metrics.evidence_chars)
                    .sum(),
                ..Default::default()
            },
            confidence: 0.0,
        }
    }
}

impl Default for OrchestratorState {
    fn default() -> Self {
        Self::new()
    }
}
