// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Orchestrator loop — multi-document retrieval via MapReduce.
//!
//! Flow:
//! 1. Fast path: find_cross → direct hit across all docs
//! 2. Analyze: ls_docs + find_cross → LLM decides which docs + tasks
//! 3. Dispatch: fan-out N SubAgents in parallel
//! 4. Integrate: merge evidence, check cross-doc sufficiency, optionally re-dispatch
//! 5. Synthesis: LLM generates final cross-doc answer

use tracing::{debug, info, warn};

use crate::llm::LlmClient;
use crate::retrieval::scoring::bm25::extract_keywords;

use super::config::{Config, Output, WorkspaceContext};
use super::context::FindHit;
use super::prompts::{
    answer_synthesis, check_sufficiency, orchestrator_analysis, orchestrator_integration,
    parse_dispatch_plan, parse_sufficiency_response, DispatchEntry, OrchestratorAnalysisParams,
    OrchestratorIntegrationParams, SynthesisParams,
};
use super::state::OrchestratorState;
use super::subagent;
use super::tools::orchestrator as orch_tools;

/// Maximum number of integration retries (supplemental dispatches).
const MAX_INTEGRATE_RETRIES: u32 = 1;

/// Run the Orchestrator loop for multi-document retrieval.
pub async fn run(
    query: &str,
    ws: &WorkspaceContext<'_>,
    config: &Config,
    llm: &LlmClient,
) -> crate::error::Result<Output> {
    info!(docs = ws.doc_count(), "Orchestrator starting");

    let mut state = OrchestratorState::new();
    let mut orch_llm_calls: u32 = 0;

    // --- Phase 0: Fast path ---
    if config.enable_fast_path {
        if let Some(output) = fast_path(query, ws, config) {
            info!("Orchestrator fast path hit");
            return Ok(output);
        }
    }

    // --- Phase 1: Analyze ---
    let doc_cards_text = orch_tools::ls_docs(ws).feedback;
    let keywords = extract_keywords(query);
    let find_text = if keywords.is_empty() {
        "(no keywords extracted)".to_string()
    } else {
        orch_tools::find_cross(&keywords, ws).feedback
    };

    info!(keywords = ?keywords, "Orchestrator analyzing");

    let (system, user) = orchestrator_analysis(&OrchestratorAnalysisParams {
        query,
        doc_cards: &doc_cards_text,
        find_results: &find_text,
    });

    let analysis_output = match llm.complete(&system, &user).await {
        Ok(output) => output,
        Err(e) => {
            warn!(error = %e, "Orchestrator analysis LLM call failed");
            // Fallback: dispatch to all documents with the original query
            return fallback_dispatch_all(query, ws, config, llm).await;
        }
    };
    orch_llm_calls += 1;

    // Check if already answered
    let dispatches = match parse_dispatch_plan(&analysis_output, ws.doc_count()) {
        Some(entries) => entries,
        None => {
            info!("Orchestrator: analysis indicates already answered");
            let mut output = Output::empty();
            output.answer = "Already answered by cross-document search.".to_string();
            return Ok(output);
        }
    };

    if dispatches.is_empty() {
        info!("Orchestrator: no relevant documents found");
        return Ok(Output::empty());
    }

    info!(
        docs = dispatches.len(),
        docs_list = ?dispatches.iter().map(|d| d.doc_idx).collect::<Vec<_>>(),
        "Orchestrator dispatching"
    );

    state.analyze_done = true;

    // --- Phase 2: Dispatch ---
    dispatch_and_collect(query, &dispatches, ws, config, llm, &mut state).await;

    // --- Phase 3: Integrate ---
    if state.all_evidence.is_empty() {
        info!("Orchestrator: no evidence collected from any SubAgent");
        return Ok(state.into_output(String::new()));
    }

    let mut retries = 0;
    while retries < MAX_INTEGRATE_RETRIES {
        // Check cross-doc sufficiency
        let evidence_summary = format_evidence_summary(&state.all_evidence);
        let sufficient = check_cross_doc_sufficiency(query, &evidence_summary, llm).await;
        orch_llm_calls += 1;

        if sufficient {
            break;
        }

        if retries < MAX_INTEGRATE_RETRIES {
            warn!(retry = retries, "Cross-doc evidence insufficient, supplementing");
            retries += 1;

            // Supplemental: do additional find_cross and dispatch to uncovered docs
            let undispatched: Vec<DispatchEntry> = (0..ws.doc_count())
                .filter(|i| !state.dispatched.contains(i))
                .take(2) // limit supplemental dispatches
                .map(|idx| DispatchEntry {
                    doc_idx: idx,
                    reason: "Supplemental dispatch".to_string(),
                    task: query.to_string(),
                })
                .collect();

            if !undispatched.is_empty() {
                dispatch_and_collect(query, &undispatched, ws, config, llm, &mut state).await;
            } else {
                break; // no more docs to dispatch
            }
        }
    }

    // Cross-doc integration via LLM
    let integration_text = format_integration_text(&state.sub_results);
    let (system, _) = orchestrator_integration(&OrchestratorIntegrationParams {
        query,
        sub_results: &[],
    });
    let integration_user = format!(
        "User question: {query}\n\nCollected evidence:\n{integration_text}\n\nIntegrated analysis:"
    );

    let integrated = match llm.complete(&system, &integration_user).await {
        Ok(output) => output,
        Err(e) => {
            warn!(error = %e, "Orchestrator integration LLM call failed");
            state
                .sub_results
                .iter()
                .map(|r| r.answer.clone())
                .collect::<Vec<_>>()
                .join("\n\n")
        }
    };
    orch_llm_calls += 1;

    // --- Phase 4: Synthesis ---
    let evidence_text = format_evidence_for_synthesis(&state.all_evidence);
    let answer = if config.enable_synthesis {
        let (sys, usr) = answer_synthesis(&SynthesisParams {
            query,
            evidence_text: &evidence_text,
            missing_info: "",
        });
        match llm.complete(&sys, &usr).await {
            Ok(a) => {
                orch_llm_calls += 1;
                a.trim().to_string()
            }
            Err(e) => {
                warn!(error = %e, "Synthesis LLM call failed, using integration output");
                integrated.trim().to_string()
            }
        }
    } else {
        integrated.trim().to_string()
    };

    let mut output = state.into_output(answer);
    output.metrics.llm_calls += orch_llm_calls;

    info!(
        evidence = output.evidence.len(),
        llm_calls = output.metrics.llm_calls,
        "Orchestrator complete"
    );

    Ok(output)
}

/// Try fast path across all documents.
fn fast_path(query: &str, ws: &WorkspaceContext<'_>, config: &Config) -> Option<Output> {
    let keywords = extract_keywords(query);
    if keywords.is_empty() {
        return None;
    }

    let cross_hits = ws.find_cross_all(&keywords);
    if cross_hits.is_empty() {
        return None;
    }

    // Find best hit across all documents
    let mut best: Option<(usize, FindHit, &crate::document::TopicEntry)> = None;
    for (doc_idx, hits) in &cross_hits {
        for hit in hits {
            for entry in &hit.entries {
                let is_better = best
                    .as_ref()
                    .map_or(true, |(_, _, best_e)| entry.weight > best_e.weight);
                if is_better && entry.weight >= config.fast_path_threshold {
                    best = Some((*doc_idx, hit.clone(), entry));
                }
            }
        }
    }

    let (doc_idx, _, best_entry) = best?;
    let doc = ws.doc(doc_idx)?;
    let content = doc.cat(best_entry.node_id).unwrap_or("").to_string();
    let title = doc.node_title(best_entry.node_id).unwrap_or("unknown").to_string();

    if content.is_empty() {
        return None;
    }

    info!(doc_idx, node = %title, weight = best_entry.weight, "Cross-doc fast path hit");

    Some(Output::fast_path(
        content.clone(),
        vec![super::config::Evidence {
            source_path: title.clone(),
            node_title: title,
            content,
            doc_name: Some(doc.doc_name.to_string()),
        }],
    ))
}

/// Dispatch SubAgents in parallel and collect results.
async fn dispatch_and_collect(
    query: &str,
    dispatches: &[DispatchEntry],
    ws: &WorkspaceContext<'_>,
    config: &Config,
    llm: &LlmClient,
    state: &mut OrchestratorState,
) {
    // Build futures for each dispatch
    let futures: Vec<_> = dispatches
        .iter()
        .filter_map(|dispatch| {
            let doc = match ws.doc(dispatch.doc_idx) {
                Some(d) => d,
                None => {
                    warn!(doc_idx = dispatch.doc_idx, "Document not found, skipping");
                    return None;
                }
            };

            state.record_dispatch(dispatch.doc_idx);

            let query = query.to_string();
            let task = dispatch.task.clone();
            let config = config.for_subagent();

            // Clone LlmClient for each sub-agent
            let llm = llm.clone();

            Some(async move {
                let result = subagent::run(&query, Some(&task), doc, &config, &llm).await;
                (dispatch.doc_idx, result)
            })
        })
        .collect();

    // Run all SubAgents concurrently
    let results: Vec<_> = futures::future::join_all(futures).await;

    for (doc_idx, result) in results {
        match result {
            Ok(output) => {
                info!(
                    doc_idx,
                    evidence = output.evidence.len(),
                    "SubAgent completed"
                );
                state.collect_result(output);
            }
            Err(e) => {
                warn!(doc_idx, error = %e, "SubAgent failed");
            }
        }
    }
}

/// Check cross-document evidence sufficiency via LLM.
async fn check_cross_doc_sufficiency(
    query: &str,
    evidence_summary: &str,
    llm: &LlmClient,
) -> bool {
    let (system, user) = check_sufficiency(query, evidence_summary);
    match llm.complete(&system, &user).await {
        Ok(response) => parse_sufficiency_response(&response),
        Err(e) => {
            warn!(error = %e, "Cross-doc sufficiency check failed, assuming sufficient");
            true // assume sufficient on error to avoid infinite retry
        }
    }
}

/// Format all sub-results for the integration prompt.
fn format_integration_text(sub_results: &[Output]) -> String {
    sub_results
        .iter()
        .enumerate()
        .map(|(i, result)| {
            let doc_name = result
                .evidence
                .first()
                .and_then(|e| e.doc_name.clone())
                .unwrap_or_else(|| format!("doc_{}", i));

            let evidence_text = result
                .evidence
                .iter()
                .map(|e| format!("[{}] {}", e.node_title, e.content))
                .collect::<Vec<_>>()
                .join("\n");

            let mut section = format!(
                "## Document: {} ({} evidence items)\n{}",
                doc_name,
                result.evidence.len(),
                evidence_text
            );
            if !result.answer.is_empty() {
                section.push_str(&format!("\nSub-answer: {}", result.answer));
            }
            section
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Format all evidence for the synthesis prompt.
fn format_evidence_for_synthesis(evidence: &[super::config::Evidence]) -> String {
    evidence
        .iter()
        .map(|e| {
            let doc = e.doc_name.as_deref().unwrap_or("unknown");
            format!("[{}] ({} at {})\n{}", e.node_title, doc, e.source_path, e.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Format evidence summary for sufficiency check.
fn format_evidence_summary(evidence: &[super::config::Evidence]) -> String {
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

/// Fallback: dispatch SubAgents to all documents with the original query.
async fn fallback_dispatch_all(
    query: &str,
    ws: &WorkspaceContext<'_>,
    config: &Config,
    llm: &LlmClient,
) -> crate::error::Result<Output> {
    warn!("Falling back to dispatch-all");

    let dispatches: Vec<DispatchEntry> = (0..ws.doc_count())
        .map(|idx| DispatchEntry {
            doc_idx: idx,
            reason: "Fallback dispatch".to_string(),
            task: query.to_string(),
        })
        .collect();

    let mut state = OrchestratorState::new();
    dispatch_and_collect(query, &dispatches, ws, config, llm, &mut state).await;

    if state.all_evidence.is_empty() {
        return Ok(state.into_output(String::new()));
    }

    // Simple synthesis
    let evidence_text = format_evidence_for_synthesis(&state.all_evidence);
    let (sys, usr) = answer_synthesis(&SynthesisParams {
        query,
        evidence_text: &evidence_text,
        missing_info: "",
    });

    let answer = match llm.complete(&sys, &usr).await {
        Ok(a) => a.trim().to_string(),
        Err(_) => format_evidence_as_answer(&state.all_evidence),
    };

    Ok(state.into_output(answer))
}

/// Format evidence as a simple answer (fallback).
fn format_evidence_as_answer(evidence: &[super::config::Evidence]) -> String {
    evidence
        .iter()
        .map(|e| {
            let doc = e.doc_name.as_deref().unwrap_or("unknown");
            format!("**{}** (from {} at {}):\n{}", e.node_title, doc, e.source_path, e.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_evidence_summary() {
        let evidence = vec![
            super::super::config::Evidence {
                source_path: "root/A".to_string(),
                node_title: "A".to_string(),
                content: "content".to_string(),
                doc_name: Some("doc1".to_string()),
            },
            super::super::config::Evidence {
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
    fn test_format_evidence_for_synthesis() {
        let evidence = vec![super::super::config::Evidence {
            source_path: "root/A".to_string(),
            node_title: "A".to_string(),
            content: "the answer".to_string(),
            doc_name: Some("my_doc".to_string()),
        }];
        let formatted = format_evidence_for_synthesis(&evidence);
        assert!(formatted.contains("[A]"));
        assert!(formatted.contains("my_doc"));
        assert!(formatted.contains("the answer"));
    }

    #[test]
    fn test_format_integration_text() {
        let output = Output {
            answer: "sub answer".to_string(),
            evidence: vec![super::super::config::Evidence {
                source_path: "root/X".to_string(),
                node_title: "X".to_string(),
                content: "x content".to_string(),
                doc_name: Some("doc_a".to_string()),
            }],
            metrics: super::super::config::Metrics::default(),
        };
        let formatted = format_integration_text(&[output]);
        assert!(formatted.contains("[X]"));
        assert!(formatted.contains("x content"));
        assert!(formatted.contains("sub answer"));
    }

    #[test]
    fn test_format_evidence_as_answer() {
        let evidence = vec![super::super::config::Evidence {
            source_path: "root/Y".to_string(),
            node_title: "Y".to_string(),
            content: "y content".to_string(),
            doc_name: Some("doc_a".to_string()),
        }];
        let formatted = format_evidence_as_answer(&evidence);
        assert!(formatted.contains("**Y**"));
        assert!(formatted.contains("doc_a"));
    }

    #[test]
    fn test_format_evidence_summary_empty() {
        let summary = format_evidence_summary(&[]);
        assert!(summary.contains("no evidence"));
    }
}
