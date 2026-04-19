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
use crate::scoring::bm25::extract_keywords;

use super::config::{Config, Output, WorkspaceContext};
use super::context::FindHit;
use super::events::EventEmitter;
use super::prompts::{
    DispatchEntry, OrchestratorAnalysisParams, check_sufficiency, orchestrator_analysis,
    parse_dispatch_plan, parse_sufficiency_response,
};
use super::state::OrchestratorState;
use super::subagent;
use super::tools::orchestrator as orch_tools;

/// Maximum number of integration retries (supplemental dispatches).
const MAX_INTEGRATE_RETRIES: u32 = 3;

/// Maximum number of documents to dispatch per supplemental retry.
const MAX_SUPPLEMENTAL_DISPATCH: usize = 3;

/// Outcome of the analyze phase (Phase 1).
enum AnalyzeOutcome {
    /// Produce dispatch entries for Phase 2.
    Proceed {
        dispatches: Vec<DispatchEntry>,
        llm_calls: u32,
    },
    /// Cross-doc search already answered the query.
    AlreadyAnswered { llm_calls: u32 },
    /// No relevant documents found after expanded analysis.
    NoResults { llm_calls: u32 },
    /// Analysis LLM call failed — caller should fallback.
    AnalysisFailed,
}

/// Run the Orchestrator loop for multi-document retrieval.
///
/// When `skip_analysis` is `true`, Phase 1 (LLM analysis of DocCards) is skipped
/// and all documents are dispatched directly. This is used when the user has
/// explicitly specified which documents to query.
pub async fn run(
    query: &str,
    ws: &WorkspaceContext<'_>,
    config: &Config,
    llm: &LlmClient,
    emitter: &EventEmitter,
    skip_analysis: bool,
) -> crate::error::Result<Output> {
    info!(
        docs = ws.doc_count(),
        skip_analysis, "Orchestrator starting"
    );
    emitter.emit_started(query, ws.doc_count() > 1);

    let mut state = OrchestratorState::new();
    let mut orch_llm_calls: u32 = 0;

    // --- Phase 0: Fast path ---
    if config.enable_fast_path {
        if let Some(output) = fast_path(query, ws, config, emitter) {
            info!("Orchestrator fast path hit — skipping dispatch");
            emitter.emit_completed(
                output.evidence.len(),
                output.metrics.llm_calls,
                output.metrics.rounds_used,
                true,  // fast_path_hit
                false, // budget_exhausted
                false, // plan_generated
                0,     // evidence_chars
            );
            return Ok(output);
        }
    }

    // --- Phase 1: Analyze ---
    let dispatches = match analyze(query, ws, config, llm, &mut state, emitter, skip_analysis).await
    {
        AnalyzeOutcome::Proceed {
            dispatches,
            llm_calls,
        } => {
            orch_llm_calls += llm_calls;
            dispatches
        }
        AnalyzeOutcome::AlreadyAnswered { llm_calls } => {
            let mut output = Output::empty();
            output.answer = "Already answered by cross-document search.".to_string();
            emitter.emit_completed(0, orch_llm_calls + llm_calls, 0, false, false, false, 0);
            return Ok(output);
        }
        AnalyzeOutcome::NoResults { llm_calls } => {
            emitter.emit_completed(0, orch_llm_calls + llm_calls, 0, false, false, false, 0);
            return Ok(Output::empty());
        }
        AnalyzeOutcome::AnalysisFailed => {
            return fallback_dispatch_all(query, ws, config, llm, emitter).await;
        }
    };

    // --- Phase 2: Dispatch ---
    if !dispatches.is_empty() {
        info!(
            docs = dispatches.len(),
            docs_list = ?dispatches.iter().map(|d| d.doc_idx).collect::<Vec<_>>(),
            "Phase 2: dispatching SubAgents"
        );
        dispatch_and_collect(query, &dispatches, ws, config, llm, &mut state, emitter).await;
    }

    // --- Phase 3: Integrate (only when analysis was done) ---
    // Skip cross-doc sufficiency checks when user specified documents.
    if state.all_evidence.is_empty() {
        info!("No evidence collected from any SubAgent");
        emitter.emit_completed(0, orch_llm_calls, 0, false, false, false, 0);
        return Ok(state.into_output(
            "I was unable to find relevant information across the available documents to answer your question.".to_string()
        ));
    }

    if !skip_analysis {
        orch_llm_calls += integrate(query, ws, config, llm, &mut state, emitter).await;
    }

    // --- Phase 4: Rerank ---
    let multi_doc = !skip_analysis || ws.doc_count() > 1;
    let rerank_result = crate::rerank::process(
        query,
        &state.all_evidence,
        config,
        llm,
        multi_doc,
        &state.sub_results,
    )
    .await;
    orch_llm_calls += rerank_result.llm_calls;
    if !rerank_result.answer.is_empty() {
        emitter.emit_synthesis(rerank_result.answer.len());
    }

    let mut output = state.into_output(rerank_result.answer);
    output.metrics.llm_calls += orch_llm_calls;
    output.score = rerank_result.score;

    emitter.emit_completed(
        output.evidence.len(),
        output.metrics.llm_calls,
        output.metrics.rounds_used,
        output.metrics.fast_path_hit,
        output.metrics.budget_exhausted,
        output.metrics.plan_generated,
        output.metrics.evidence_chars,
    );

    info!(
        evidence = output.evidence.len(),
        llm_calls = output.metrics.llm_calls,
        "Orchestrator complete"
    );

    Ok(output)
}

/// Phase 1: Analyze documents and produce a dispatch plan.
///
/// When `skip_analysis` is true, returns dispatch entries for all documents.
/// When false, uses LLM to analyze DocCards and keyword hits, with an
/// expanded analysis fallback if the initial pass produces no dispatches.
///
/// May mutate `state` during expanded analysis (dispatches SubAgents directly).
async fn analyze(
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
        return AnalyzeOutcome::Proceed {
            dispatches,
            llm_calls: 0,
        };
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

    // Check if already answered
    let dispatches = match parse_dispatch_plan(&analysis_output, ws.doc_count()) {
        Some(entries) => entries,
        None => {
            info!("Orchestrator: analysis indicates already answered");
            return AnalyzeOutcome::AlreadyAnswered { llm_calls };
        }
    };

    info!(dispatches = dispatches.len(), "Phase 1: parsed dispatch plan");

    if dispatches.is_empty() {
        // Expanded analysis: retry with richer context
        info!("No dispatches from initial analysis — retrying with expanded context");
        let expanded_find = format_expanded_find_context(query, ws);
        let (system, user) = expanded_analysis_prompt(query, &doc_cards_text, &expanded_find);

        match llm.complete(&system, &user).await {
            Ok(second_output) => {
                llm_calls += 1;
                info!(
                    response_len = second_output.len(),
                    response = %if second_output.len() > 500 { &second_output[..500] } else { &second_output },
                    "Phase 1 (expanded): second analysis LLM response"
                );
                if let Some(second_dispatches) = parse_dispatch_plan(&second_output, ws.doc_count())
                {
                    if !second_dispatches.is_empty() {
                        info!(
                            docs = second_dispatches.len(),
                            "Second analysis produced dispatches"
                        );
                        state.analyze_done = true;
                        dispatch_and_collect(
                            query,
                            &second_dispatches,
                            ws,
                            config,
                            llm,
                            state,
                            emitter,
                        )
                        .await;
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Second analysis LLM call failed");
            }
        }

        if state.all_evidence.is_empty() {
            info!("No relevant documents found after expanded analysis");
            return AnalyzeOutcome::NoResults { llm_calls };
        }

        // Already dispatched during expanded analysis, skip Phase 2
        return AnalyzeOutcome::Proceed {
            dispatches: Vec::new(),
            llm_calls,
        };
    }

    state.analyze_done = true;
    AnalyzeOutcome::Proceed {
        dispatches,
        llm_calls,
    }
}

/// Phase 3: Cross-doc sufficiency integration.
///
/// Checks if evidence from dispatched SubAgents is sufficient.
/// If not, supplements by dispatching additional SubAgents to
/// undispatched documents.
///
/// Returns the number of orchestrator-level LLM calls made.
async fn integrate(
    query: &str,
    ws: &WorkspaceContext<'_>,
    config: &Config,
    llm: &LlmClient,
    state: &mut OrchestratorState,
    emitter: &EventEmitter,
) -> u32 {
    info!(
        evidence = state.all_evidence.len(),
        sub_results = state.sub_results.len(),
        "Phase 3: integrating cross-doc evidence"
    );

    let mut llm_calls: u32 = 0;

    let mut retries = 0;
    while retries < MAX_INTEGRATE_RETRIES {
        let evidence_summary = format_evidence_summary(&state.all_evidence);
        let sufficient = check_cross_doc_sufficiency(query, &evidence_summary, llm).await;
        llm_calls += 1;
        info!(
            sufficient,
            evidence = state.all_evidence.len(),
            retry = retries,
            "Cross-doc sufficiency check"
        );
        emitter.emit_sufficiency(sufficient, state.all_evidence.len());

        if sufficient {
            break;
        }

        warn!(
            retry = retries,
            "Cross-doc evidence insufficient, supplementing"
        );
        retries += 1;

        let max_dispatch = MAX_SUPPLEMENTAL_DISPATCH.min(ws.doc_count() - state.dispatched.len());
        let undispatched: Vec<DispatchEntry> = (0..ws.doc_count())
            .filter(|i| !state.dispatched.contains(i))
            .take(max_dispatch)
            .map(|idx| DispatchEntry {
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

/// Try fast path across all documents.
fn fast_path(
    query: &str,
    ws: &WorkspaceContext<'_>,
    config: &Config,
    emitter: &EventEmitter,
) -> Option<Output> {
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
    let title = doc
        .node_title(best_entry.node_id)
        .unwrap_or("unknown")
        .to_string();

    if content.is_empty() {
        return None;
    }

    info!(doc_idx, node = %title, weight = best_entry.weight, "Cross-doc fast path hit");

    emitter.emit_fast_path(&keywords.join(","), &title, best_entry.weight);

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
    emitter: &EventEmitter,
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
            let doc_idx = dispatch.doc_idx;
            let doc_name = doc.doc_name.to_string();

            // Clone LlmClient for each sub-agent
            let llm = llm.clone();

            // Each SubAgent gets a noop emitter (orchestrator emits its own events)
            let sub_emitter = EventEmitter::noop();

            Some(async move {
                emitter.emit_subagent_dispatched(doc_idx, &doc_name, &task);
                let result =
                    subagent::run(&query, Some(&task), doc, &config, &llm, &sub_emitter).await;
                (doc_idx, result)
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
                emitter.emit_subagent_completed(doc_idx, output.evidence.len(), true);
                state.collect_result(output);
            }
            Err(e) => {
                warn!(doc_idx, error = %e, "SubAgent failed");
                emitter.emit_subagent_completed(doc_idx, 0, false);
            }
        }
    }
}

/// Check cross-document evidence sufficiency via LLM.
async fn check_cross_doc_sufficiency(query: &str, evidence_summary: &str, llm: &LlmClient) -> bool {
    let (system, user) = check_sufficiency(query, evidence_summary);
    match llm.complete(&system, &user).await {
        Ok(response) => parse_sufficiency_response(&response),
        Err(e) => {
            warn!(error = %e, "Cross-doc sufficiency check failed, assuming sufficient");
            true // assume sufficient on error to avoid infinite retry
        }
    }
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

/// Fallback: dispatch SubAgents to all documents with the original query.
async fn fallback_dispatch_all(
    query: &str,
    ws: &WorkspaceContext<'_>,
    config: &Config,
    llm: &LlmClient,
    emitter: &EventEmitter,
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
    dispatch_and_collect(query, &dispatches, ws, config, llm, &mut state, emitter).await;

    if state.all_evidence.is_empty() {
        emitter.emit_completed(0, 0, 0, false, false, false, 0);
        return Ok(state.into_output(String::new()));
    }

    // Use rerank pipeline for synthesis
    let multi_doc = ws.doc_count() > 1;
    let rerank_result = crate::rerank::process(
        query,
        &state.all_evidence,
        config,
        llm,
        multi_doc,
        &state.sub_results,
    )
    .await;
    if !rerank_result.answer.is_empty() {
        emitter.emit_synthesis(rerank_result.answer.len());
    }

    let mut output = state.into_output(rerank_result.answer);
    output.metrics.llm_calls += rerank_result.llm_calls;
    output.score = rerank_result.score;

    emitter.emit_completed(
        output.evidence.len(),
        output.metrics.llm_calls,
        output.metrics.rounds_used,
        output.metrics.fast_path_hit,
        output.metrics.budget_exhausted,
        output.metrics.plan_generated,
        output.metrics.evidence_chars,
    );
    Ok(output)
}

/// Format per-document keyword hit details for the expanded analysis prompt.
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
        let doc_name = doc.doc_name;
        output.push_str(&format!(
            "Document [{}] {} keyword matches:\n",
            doc_idx + 1,
            doc_name
        ));
        for hit in &hits {
            for entry in &hit.entries {
                let title = doc.node_title(entry.node_id).unwrap_or("?");
                let summary = doc
                    .nav_entry(entry.node_id)
                    .map(|e| e.overview.as_str())
                    .unwrap_or("");
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

    if output.is_empty() {
        "(no keyword matches across documents)".to_string()
    } else {
        output
    }
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
    fn test_format_evidence_summary_empty() {
        let summary = format_evidence_summary(&[]);
        assert!(summary.contains("no evidence"));
    }
}
