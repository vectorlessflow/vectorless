// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Rerank result types.

use serde::{Deserialize, Serialize};

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

/// Output from the rerank pipeline.
pub struct RerankOutput {
    /// Synthesized answer.
    pub answer: String,
    /// Number of LLM calls used during synthesis/fusion.
    pub llm_calls: u32,
    /// Confidence score (0.0–1.0) — derived from LLM evaluate() result.
    pub confidence: f32,
}
