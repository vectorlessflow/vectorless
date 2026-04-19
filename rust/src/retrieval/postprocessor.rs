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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Evidence, Metrics};

    fn make_output(answer: &str, evidence_count: usize) -> Output {
        let evidence: Vec<Evidence> = (0..evidence_count)
            .map(|i| Evidence {
                source_path: format!("path/{}", i),
                node_title: format!("Node {}", i),
                content: format!("Content {}", i),
                doc_name: None,
            })
            .collect();

        Output {
            answer: answer.to_string(),
            evidence,
            metrics: Metrics::default(),
        }
    }

    #[test]
    fn single_result_with_answer() {
        let output = make_output("The answer is 42", 1);
        let result = to_single_result(&output);
        assert_eq!(result.content, "The answer is 42");
        assert_eq!(result.score, 0.8);
    }

    #[test]
    fn single_result_without_answer() {
        let output = make_output("", 2);
        let result = to_single_result(&output);
        assert!(result.content.contains("Node 0"));
        assert!(result.content.contains("Node 1"));
    }

    #[test]
    fn empty_evidence_is_zero_score() {
        let output = make_output("", 0);
        let result = to_single_result(&output);
        assert_eq!(result.score, 0.0);
    }

    #[test]
    fn multi_result_uses_answer() {
        let output = make_output("Combined answer", 3);
        let result = to_multi_result(&output);
        assert_eq!(result.content, "Combined answer");
    }
}
