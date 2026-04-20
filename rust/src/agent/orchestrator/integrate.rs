// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Phase 3: Cross-doc sufficiency integration.

use tracing::{info, warn};

use crate::llm::LlmClient;

use super::super::config::{AgentConfig, Evidence, WorkspaceContext};
use super::super::events::EventEmitter;
use super::super::prompts::{check_sufficiency, parse_sufficiency_response};
use super::super::state::OrchestratorState;
use super::dispatch::dispatch_and_collect;

/// Check cross-doc sufficiency and supplement if needed.
///
/// Returns the number of orchestrator-level LLM calls made.
pub async fn integrate(
    query: &str,
    ws: &WorkspaceContext<'_>,
    config: &AgentConfig,
    llm: &LlmClient,
    state: &mut OrchestratorState,
    emitter: &EventEmitter,
) -> u32 {
    let max_retries = config.orchestrator.max_integration_retries;
    let max_supplemental = config.orchestrator.max_supplemental_docs;

    info!(
        evidence = state.all_evidence.len(),
        sub_results = state.sub_results.len(),
        "Phase 3: integrating cross-doc evidence"
    );

    let mut llm_calls: u32 = 0;
    let mut retries = 0;

    while retries < max_retries {
        let evidence_summary = format_evidence_summary(&state.all_evidence);
        let sufficient = check_cross_doc_sufficiency(query, &evidence_summary, llm).await;
        llm_calls += 1;

        info!(
            sufficient, evidence = state.all_evidence.len(), retry = retries,
            "Cross-doc sufficiency check"
        );
        emitter.emit_orchestrator_evaluated(sufficient, state.all_evidence.len(), None);

        if sufficient {
            break;
        }

        warn!(retry = retries, "Cross-doc evidence insufficient, supplementing");
        retries += 1;

        let max_dispatch = max_supplemental.min(ws.doc_count() - state.dispatched.len());
        let undispatched: Vec<super::super::prompts::DispatchEntry> = (0..ws.doc_count())
            .filter(|i| !state.dispatched.contains(i))
            .take(max_dispatch)
            .map(|idx| super::super::prompts::DispatchEntry {
                doc_idx: idx,
                reason: "Supplemental dispatch".to_string(),
                task: query.to_string(),
            })
            .collect();

        if !undispatched.is_empty() {
            dispatch_and_collect(query, &undispatched, ws, config, llm, state, emitter).await;
        } else {
            break;
        }
    }

    llm_calls
}

/// Check cross-document evidence sufficiency via LLM.
async fn check_cross_doc_sufficiency(query: &str, evidence_summary: &str, llm: &LlmClient) -> bool {
    let (system, user) = check_sufficiency(query, evidence_summary);
    match llm.complete(&system, &user).await {
        Ok(response) => parse_sufficiency_response(&response),
        Err(e) => {
            warn!(error = %e, "Cross-doc sufficiency check failed, assuming sufficient");
            true
        }
    }
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
            format!("- [{}] (from {}) {} chars", e.node_title, doc, e.content.len())
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
