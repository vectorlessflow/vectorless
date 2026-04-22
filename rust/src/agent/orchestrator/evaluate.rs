// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Evaluate cross-document evidence sufficiency via LLM.
//!
//! Replaces the old `integrate` module's heuristic sufficiency check.
//! LLM errors propagate — no silent "assume sufficient" fallback.

use tracing::info;

use crate::error::Error;
use crate::llm::LlmClient;

use super::super::config::Evidence;
use super::super::prompts::{check_sufficiency, parse_sufficiency_response};

/// Result of the evidence sufficiency evaluation.
pub struct EvalResult {
    /// Whether the collected evidence is sufficient to answer the query.
    pub sufficient: bool,
    /// Description of what information is still missing (empty if sufficient).
    pub missing_info: String,
}

/// Evaluate cross-document evidence sufficiency via LLM.
///
/// Propagates LLM errors as [`Error::LlmReasoning`].
/// The caller decides how to handle insufficiency (replan, abort, etc.).
pub async fn evaluate(
    query: &str,
    evidence: &[Evidence],
    llm: &LlmClient,
) -> crate::error::Result<EvalResult> {
    let evidence_summary = format_evidence_summary(evidence);
    let (system, user) = check_sufficiency(query, &evidence_summary);

    info!(
        evidence = evidence.len(),
        "Evaluating evidence sufficiency..."
    );
    let response = llm
        .complete(&system, &user)
        .await
        .map_err(|e| Error::LlmReasoning {
            stage: "orchestrator/evaluate".to_string(),
            detail: format!("Sufficiency check LLM call failed: {e}"),
        })?;

    let sufficient = parse_sufficiency_response(&response);
    let missing_info = if sufficient {
        String::new()
    } else {
        // Extract the reason from the response (everything after SUFFICIENT/INSUFFICIENT)
        let reason = response
            .trim()
            .strip_prefix("INSUFFICIENT")
            .or_else(|| response.trim().strip_prefix("Insufficient"))
            .unwrap_or("")
            .trim_start_matches(|c: char| c == '-' || c == ' ' || c == ':');
        if reason.is_empty() {
            "Evidence does not fully address the query.".to_string()
        } else {
            reason.to_string()
        }
    };

    info!(
        sufficient,
        evidence = evidence.len(),
        missing_info_len = missing_info.len(),
        "Cross-doc sufficiency evaluation"
    );

    Ok(EvalResult {
        sufficient,
        missing_info,
    })
}

/// Format evidence summary for sufficiency check.
pub fn format_evidence_summary(evidence: &[Evidence]) -> String {
    if evidence.is_empty() {
        return "(no evidence)".to_string();
    }
    evidence
        .iter()
        .map(|e| {
            let doc = e.doc_name.as_deref().unwrap_or("unknown");
            format!(
                "- [{}] (from {}) {} chars",
                e.node_title,
                doc,
                e.content.len()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_evidence_summary() {
        let evidence = vec![
            Evidence {
                source_path: "root/A".to_string(),
                node_title: "A".to_string(),
                content: "content".to_string(),
                doc_name: Some("doc1".to_string()),
            },
            Evidence {
                source_path: "root/B".to_string(),
                node_title: "B".to_string(),
                content: "more content".to_string(),
                doc_name: Some("doc2".to_string()),
            },
        ];
        let summary = format_evidence_summary(&evidence);
        assert!(summary.contains("[A]"));
        assert!(summary.contains("doc1"));
        assert!(summary.contains("[B]"));
        assert!(summary.contains("doc2"));
    }

    #[test]
    fn test_format_evidence_summary_empty() {
        let summary = format_evidence_summary(&[]);
        assert!(summary.contains("no evidence"));
    }
}
