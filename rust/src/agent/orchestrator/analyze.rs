// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Phase 1: Analyze documents and produce a dispatch plan.

use tracing::{debug, info, warn};

use crate::llm::LlmClient;
use crate::scoring::bm25::extract_keywords;

use super::super::config::{Config, WorkspaceContext};
use super::super::events::EventEmitter;
use super::super::prompts::{DispatchEntry, OrchestratorAnalysisParams, orchestrator_analysis, parse_dispatch_plan};
use super::super::state::OrchestratorState;
use super::super::tools::orchestrator as orch_tools;
use super::dispatch::dispatch_and_collect;

/// Outcome of the analyze phase.
pub enum AnalyzeOutcome {
    /// Produce dispatch entries for Phase 2.
    Proceed { dispatches: Vec<DispatchEntry>, llm_calls: u32 },
    /// Cross-doc search already answered the query.
    AlreadyAnswered { llm_calls: u32 },
    /// No relevant documents found.
    NoResults { llm_calls: u32 },
    /// Analysis LLM call failed — caller should fallback.
    AnalysisFailed,
}

/// Analyze documents and produce a dispatch plan.
pub async fn analyze(
    query: &str,
    ws: &WorkspaceContext<'_>,
    config: &Config,
    llm: &LlmClient,
    state: &mut OrchestratorState,
    emitter: &EventEmitter,
    skip_analysis: bool,
) -> AnalyzeOutcome {
    if skip_analysis {
        debug!("Phase 1: skipping (user-specified documents)");
        let dispatches = (0..ws.doc_count())
            .map(|idx| DispatchEntry {
                doc_idx: idx,
                reason: "User-specified document".to_string(),
                task: query.to_string(),
            })
            .collect();
        return AnalyzeOutcome::Proceed { dispatches, llm_calls: 0 };
    }

    debug!("Phase 1: analyzing doc cards and cross-doc keywords");
    let mut llm_calls: u32 = 0;

    let doc_cards_text = orch_tools::ls_docs(ws).feedback;
    let keywords = extract_keywords(query);
    let find_text = if keywords.is_empty() {
        "(no keywords extracted)".to_string()
    } else {
        orch_tools::find_cross(&keywords, ws).feedback
    };

    info!(keywords = ?keywords, "Phase 1: analyzing");
    debug!(
        doc_cards_len = doc_cards_text.len(),
        find_results_len = find_text.len(),
        "Phase 1: analysis input"
    );

    let (system, user) = orchestrator_analysis(&OrchestratorAnalysisParams {
        query,
        doc_cards: &doc_cards_text,
        find_results: &find_text,
    });

    let analysis_output = match llm.complete(&system, &user).await {
        Ok(output) => output,
        Err(e) => {
            warn!(error = %e, "Orchestrator analysis LLM call failed");
            emitter.emit_error(&e.to_string());
            return AnalyzeOutcome::AnalysisFailed;
        }
    };
    llm_calls += 1;

    info!(
        response_len = analysis_output.len(),
        response = %if analysis_output.len() > 500 { &analysis_output[..500] } else { &analysis_output },
        "Phase 1: analysis LLM response"
    );

    let dispatches = match parse_dispatch_plan(&analysis_output, ws.doc_count()) {
        Some(entries) => entries,
        None => {
            info!("Orchestrator: analysis indicates already answered");
            return AnalyzeOutcome::AlreadyAnswered { llm_calls };
        }
    };

    info!(dispatches = dispatches.len(), "Phase 1: parsed dispatch plan");

    if dispatches.is_empty() {
        return expanded_analysis(query, ws, config, llm, state, emitter, &doc_cards_text, llm_calls).await;
    }

    state.analyze_done = true;
    AnalyzeOutcome::Proceed { dispatches, llm_calls }
}

/// Retry analysis with expanded keyword context.
async fn expanded_analysis(
    query: &str,
    ws: &WorkspaceContext<'_>,
    config: &Config,
    llm: &LlmClient,
    state: &mut OrchestratorState,
    emitter: &EventEmitter,
    doc_cards_text: &str,
    mut llm_calls: u32,
) -> AnalyzeOutcome {
    info!("No dispatches from initial analysis — retrying with expanded context");
    let expanded_find = format_expanded_find_context(query, ws);
    let (system, user) = expanded_analysis_prompt(query, doc_cards_text, &expanded_find);

    match llm.complete(&system, &user).await {
        Ok(second_output) => {
            llm_calls += 1;
            info!(
                response_len = second_output.len(),
                response = %if second_output.len() > 500 { &second_output[..500] } else { &second_output },
                "Phase 1 (expanded): second analysis LLM response"
            );
            if let Some(second_dispatches) = parse_dispatch_plan(&second_output, ws.doc_count()) {
                if !second_dispatches.is_empty() {
                    info!(docs = second_dispatches.len(), "Second analysis produced dispatches");
                    state.analyze_done = true;
                    dispatch_and_collect(query, &second_dispatches, ws, config, llm, state, emitter).await;
                }
            }
        }
        Err(e) => {
            warn!(error = %e, "Second analysis LLM call failed");
        }
    }

    if state.all_evidence.is_empty() {
        AnalyzeOutcome::NoResults { llm_calls }
    } else {
        AnalyzeOutcome::Proceed { dispatches: Vec::new(), llm_calls }
    }
}

/// Format per-document keyword hit details for expanded analysis.
fn format_expanded_find_context(query: &str, ws: &WorkspaceContext<'_>) -> String {
    let keywords = extract_keywords(query);
    if keywords.is_empty() {
        return "(no keywords to search)".to_string();
    }

    let mut output = String::new();
    for (doc_idx, doc) in ws.docs.iter().enumerate() {
        let hits = doc.find_all(&keywords);
        if hits.is_empty() {
            continue;
        }
        output.push_str(&format!("Document [{}] {} keyword matches:\n", doc_idx + 1, doc.doc_name));
        for hit in &hits {
            for entry in &hit.entries {
                let title = doc.node_title(entry.node_id).unwrap_or("?");
                let summary = doc.nav_entry(entry.node_id).map(|e| e.overview.as_str()).unwrap_or("");
                output.push_str(&format!(
                    "  keyword '{}' → {} (depth {}, weight {:.2})",
                    hit.keyword, title, entry.depth, entry.weight
                ));
                if !summary.is_empty() {
                    output.push_str(&format!(" — {}", summary));
                }
                output.push('\n');
            }
        }
        output.push('\n');
    }

    if output.is_empty() { "(no keyword matches across documents)".to_string() } else { output }
}

/// Build the expanded analysis prompt for the second LLM pass.
fn expanded_analysis_prompt(query: &str, doc_cards: &str, expanded_find: &str) -> (String, String) {
    let system =
        "You are a multi-document retrieval coordinator. The initial analysis did not identify \
         relevant documents. Review the detailed keyword matching results below and reconsider \
         which documents may contain relevant information.

Output format — for each relevant document, output a block:
- doc: <number>
  reason: <why this document is relevant>
  task: <what specific information to find in this document>

Only include documents that are likely to contain relevant information."
            .to_string();

    let user = format!(
        "Available documents:\n{doc_cards}\n\n\
         Detailed keyword matching results:\n{expanded_find}\n\n\
         User question: {query}\n\n\
         Relevant documents:"
    );

    (system, user)
}
