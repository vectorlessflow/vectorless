// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration and output types for the retrieval agent.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Worker configuration
// ---------------------------------------------------------------------------

/// Worker configuration — navigation budget settings.
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Maximum navigation rounds per Worker loop (ls/cd/cat/grep/head/find etc.).
    /// `check` does NOT count against this budget.
    pub max_rounds: u32,
    /// Hard cap on total LLM calls per Worker (planning + nav + check).
    /// Prevents runaway costs regardless of max_rounds. 0 = no limit.
    pub max_llm_calls: u32,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            max_rounds: 100,
            max_llm_calls: 200,
        }
    }
}

impl WorkerConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// Answer pipeline configuration
// ---------------------------------------------------------------------------

/// Answer pipeline configuration — synthesis settings.
#[derive(Debug, Clone)]
pub struct AnswerConfig {
    /// Maximum number of evidence items to feed into synthesis.
    pub evidence_cap: usize,
}

impl Default for AnswerConfig {
    fn default() -> Self {
        Self { evidence_cap: 20 }
    }
}

// ---------------------------------------------------------------------------
// Aggregated agent configuration
// ---------------------------------------------------------------------------

/// Aggregated configuration for the entire retrieval agent system.
#[derive(Debug, Clone, Default)]
pub struct AgentConfig {
    pub worker: WorkerConfig,
    pub answer: AnswerConfig,
}

impl AgentConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// Agent output — the final result of a retrieval operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    /// Final synthesized answer.
    pub answer: String,
    /// Collected evidence from navigation.
    pub evidence: Vec<Evidence>,
    /// Agent execution metrics.
    pub metrics: Metrics,
    /// Confidence score (0.0–1.0) — derived from LLM evaluate() result.
    pub confidence: f32,
    /// Reasoning trace steps collected during agent navigation.
    pub trace_steps: Vec<vectorless_document::TraceStep>,
}

impl Output {
    /// Create an empty output (no evidence found).
    pub fn empty() -> Self {
        Self {
            answer: String::new(),
            evidence: Vec::new(),
            metrics: Metrics::default(),
            confidence: 0.0,
            trace_steps: Vec::new(),
        }
    }
}

/// A single piece of evidence collected during navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Navigation path where this evidence was found (e.g., "Root/API Reference/Auth").
    pub source_path: String,
    /// Title of the node.
    pub node_title: String,
    /// Content of the node.
    pub content: String,
    /// Source document name (set by Orchestrator in multi-doc scenarios).
    pub doc_name: Option<String>,
}

/// Agent execution metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub rounds_used: u32,
    pub llm_calls: u32,
    pub nodes_visited: usize,
    pub budget_exhausted: bool,
    pub plan_generated: bool,
    pub check_count: u32,
    pub evidence_chars: usize,
}

/// Step result from the navigation loop.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Continue to next round with the given feedback.
    Continue,
    /// Navigation is done, proceed to synthesis.
    Done,
    /// Forced done due to budget exhaustion or error.
    ForceDone(String),
}

// ---------------------------------------------------------------------------
// Worker output (evidence only, no answer)
// ---------------------------------------------------------------------------

/// Output from a single Worker — pure evidence, no answer synthesis.
/// Rerank handles all answer generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerOutput {
    /// Collected evidence from document navigation.
    pub evidence: Vec<Evidence>,
    /// Worker execution metrics.
    pub metrics: WorkerMetrics,
    /// Document name this Worker was assigned to.
    pub doc_name: String,
    /// Reasoning trace steps from this Worker.
    pub trace_steps: Vec<vectorless_document::TraceStep>,
}

/// Metrics specific to a single Worker's execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkerMetrics {
    /// Number of navigation rounds used.
    pub rounds_used: u32,
    /// Number of LLM calls made.
    pub llm_calls: u32,
    /// Number of distinct nodes visited.
    pub nodes_visited: usize,
    /// Whether the LLM call budget was exhausted.
    pub budget_exhausted: bool,
    /// Whether a navigation plan was generated.
    pub plan_generated: bool,
    /// Number of times `check` was called.
    pub check_count: u32,
    /// Total characters of collected evidence.
    pub evidence_chars: usize,
}

impl From<WorkerOutput> for Output {
    fn from(wo: WorkerOutput) -> Self {
        Output {
            answer: String::new(),
            evidence: wo.evidence,
            metrics: Metrics {
                rounds_used: wo.metrics.rounds_used,
                llm_calls: wo.metrics.llm_calls,
                nodes_visited: wo.metrics.nodes_visited,
                budget_exhausted: wo.metrics.budget_exhausted,
                plan_generated: wo.metrics.plan_generated,
                check_count: wo.metrics.check_count,
                evidence_chars: wo.metrics.evidence_chars,
            },
            confidence: 0.0,
            trace_steps: wo.trace_steps,
        }
    }
}

// ---------------------------------------------------------------------------
// Scope types
// ---------------------------------------------------------------------------

/// Scope context — determines which path the dispatcher takes.
///
/// Both variants go through the Orchestrator. The difference is:
/// - `Specified`: user chose specific documents → skip Orchestrator analysis phase
/// - `Workspace`: user didn't specify → Orchestrator analyzes DocCards to select docs
pub enum Scope<'a> {
    /// User specified one or more documents (by doc_id).
    /// Orchestrator skips analysis, spawns Workers directly.
    Specified(Vec<DocContext<'a>>),
    /// Workspace scope — user didn't specify documents.
    /// Orchestrator analyzes DocCards and selects relevant ones.
    Workspace(WorkspaceContext<'a>),
}

/// Read-only access to a single document's compile artifacts.
pub struct DocContext<'a> {
    /// Document content tree.
    pub tree: &'a vectorless_document::DocumentTree,
    /// Navigation index (includes DocCard).
    pub nav_index: &'a vectorless_document::NavigationIndex,
    /// Reasoning index (keyword/topic lookup).
    pub reasoning_index: &'a vectorless_document::ReasoningIndex,
    /// Document name (for evidence source attribution).
    pub doc_name: &'a str,
}

/// Read-only access to multiple documents' compile artifacts.
pub struct WorkspaceContext<'a> {
    /// All available documents.
    pub docs: Vec<DocContext<'a>>,
}

impl<'a> WorkspaceContext<'a> {
    /// Create a workspace from a slice of DocContexts.
    pub fn new(docs: Vec<DocContext<'a>>) -> Self {
        Self { docs }
    }

    /// Number of documents in the workspace.
    pub fn doc_count(&self) -> usize {
        self.docs.len()
    }

    /// Whether the workspace has only one document.
    pub fn is_single(&self) -> bool {
        self.docs.len() == 1
    }
}
