// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Post-processing of agent output into client-facing results.
//!
//! Converts raw agent [`Output`] into [`QueryResultItem`]. Future home
//! of rerank/dedup/fusion logic (Phase 4).

use crate::agent::Output;
use crate::client::QueryResultItem;

/// Convert agent output to a client query result (single document).
pub fn to_single_result(output: &Output) -> QueryResultItem {
    let node_ids: Vec<String> = output
        .evidence
        .iter()
        .map(|e| e.source_path.clone())
        .collect();

    let content = if output.answer.is_empty() {
        output
            .evidence
            .iter()
            .map(|e| format!("## {}\n{}", e.node_title, e.content))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    } else {
        output.answer.clone()
    };

    let score = if output.evidence.is_empty() { 0.0 } else { 0.8 };

    QueryResultItem {
        doc_id: String::new(), // Set by caller
        node_ids,
        content,
        score,
    }
}

/// Convert agent output to a client query result (multi-document).
pub fn to_multi_result(output: &Output) -> QueryResultItem {
    let node_ids: Vec<String> = output
        .evidence
        .iter()
        .map(|e| e.source_path.clone())
        .collect();

    QueryResultItem {
        doc_id: String::new(),
        node_ids,
        content: output.answer.clone(),
        score: if output.evidence.is_empty() { 0.0 } else { 0.8 },
    }
}