// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration and output types for the retrieval agent.

use serde::{Deserialize, Serialize};

/// Agent configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Maximum navigation rounds per SubAgent loop (ls/cd/cat/grep/head/find etc.).
    /// `check` does NOT count against this budget.
    pub max_rounds: u32,
    /// Hard cap on total LLM calls per SubAgent (planning + nav + check + synthesis).
    /// Prevents runaway costs regardless of max_rounds. 0 = no limit.
    pub max_llm_calls: u32,
    /// Enable fast-path (keyword lookup before full navigation).
    pub enable_fast_path: bool,
    /// Enable answer synthesis after evidence collection.
    pub enable_synthesis: bool,
    /// Confidence threshold for fast-path direct hit.
    pub fast_path_threshold: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_rounds: 8,
            max_llm_calls: 15,
            enable_fast_path: true,
            enable_synthesis: true,
            fast_path_threshold: 0.85,
        }
    }
}

impl Config {
    /// Create a new config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Derive a SubAgent-specific config (used by Orchestrator for dispatched agents).
    pub fn for_subagent(&self) -> Self {
        Self {
            max_rounds: self.max_rounds,
            max_llm_calls: self.max_llm_calls,
            enable_fast_path: self.enable_fast_path,
            enable_synthesis: true,
            fast_path_threshold: self.fast_path_threshold,
        }
    }
}

/// Agent output — the final result of a retrieval operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    /// Final synthesized answer (may be empty if synthesis is disabled).
    pub answer: String,
    /// Collected evidence from navigation.
    pub evidence: Vec<Evidence>,
    /// Agent execution metrics.
    pub metrics: Metrics,
}

impl Output {
    /// Create an output from fast-path (no navigation loop).
    pub fn fast_path(answer: String, evidence: Vec<Evidence>) -> Self {
        Self {
            answer,
            evidence,
            metrics: Metrics {
                fast_path_hit: true,
                ..Default::default()
            },
        }
    }

    /// Create an empty output (no evidence found).
    pub fn empty() -> Self {
        Self {
            answer: String::new(),
            evidence: Vec::new(),
            metrics: Metrics::default(),
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
    /// Number of navigation rounds used (ls/cd/cat/grep etc., excludes check).
    pub rounds_used: u32,
    /// Number of LLM calls made (includes planning + nav + check + synthesis).
    pub llm_calls: u32,
    /// Number of distinct nodes visited.
    pub nodes_visited: usize,
    /// Whether the fast-path was hit.
    pub fast_path_hit: bool,
    /// Whether the LLM call budget was exhausted.
    pub budget_exhausted: bool,
    /// Whether a navigation plan was generated (Phase 1.5).
    pub plan_generated: bool,
    /// Number of times `check` was called.
    pub check_count: u32,
    /// Total characters of collected evidence.
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

/// Scope context — determines which path the dispatcher takes.
///
/// Both variants go through the Orchestrator. The difference is:
/// - `Specified`: user chose specific documents → skip Orchestrator analysis phase
/// - `Workspace`: user didn't specify → Orchestrator analyzes DocCards to select docs
pub enum Scope<'a> {
    /// User specified one or more documents (by doc_id).
    /// Orchestrator skips analysis, spawns SubAgents directly.
    Specified(Vec<DocContext<'a>>),
    /// Workspace scope — user didn't specify documents.
    /// Orchestrator analyzes DocCards and selects relevant ones.
    Workspace(WorkspaceContext<'a>),
}

/// Read-only access to a single document's compile artifacts.
pub struct DocContext<'a> {
    /// Document content tree.
    pub tree: &'a crate::document::DocumentTree,
    /// Navigation index (includes DocCard).
    pub nav_index: &'a crate::document::NavigationIndex,
    /// Reasoning index (keyword/topic lookup).
    pub reasoning_index: &'a crate::document::ReasoningIndex,
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
