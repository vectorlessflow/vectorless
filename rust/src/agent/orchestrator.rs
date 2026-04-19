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
use crate::utils::bm25::extract_keywords;

use super::config::{Config, Output, WorkspaceContext};
use super::context::FindHit;
use super::events::EventEmitter;
use super::prompts::{
    DispatchEntry, OrchestratorAnalysisParams, OrchestratorIntegrationParams, SynthesisParams,
    answer_synthesis, check_sufficiency, orchestrator_analysis, orchestrator_integration,
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
    info!(docs = ws.doc_count(), skip_analysis, "Orchestrator starting");
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
    let dispatches = match analyze(query, ws, config, llm, &mut state, emitter, skip_analysis).await {
        AnalyzeOutcome::Proceed { dispatches, llm_calls } => {
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
        orch_llm_calls +=
            integrate(query, ws, config, llm, &mut state, emitter).await;
    }

    // --- Phase 4: Synthesize ---
    let (answer, synth_calls) =
        synthesize(query, ws, config, llm, &state, emitter, skip_analysis).await;
    orch_llm_calls += synth_calls;

    let mut output = state.into_output(answer);
    output.metrics.llm_calls += orch_llm_calls;

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

    // Check if already answered
    let dispatches = match parse_dispatch_plan(&analysis_output, ws.doc_count()) {
        Some(entries) => entries,
        None => {
            info!("Orchestrator: analysis indicates already answered");
            return AnalyzeOutcome::AlreadyAnswered { llm_calls };
        }
    };

    if dispatches.is_empty() {
        // Expanded analysis: retry with richer context
        info!("No dispatches from initial analysis — retrying with expanded context");
        let expanded_find = format_expanded_find_context(query, ws);
        let (system, user) = expanded_analysis_prompt(query, &doc_cards_text, &expanded_find);

        match llm.complete(&system, &user).await {
            Ok(second_output) => {
                llm_calls += 1;
                if let Some(second_dispatches) =
                    parse_dispatch_plan(&second_output, ws.doc_count())
                {
                    if !second_dispatches.is_empty() {
                        info!(
                            docs = second_dispatches.len(),
                            "Second analysis produced dispatches"
                        );
                        state.analyze_done = true;
                        dispatch_and_collect(
                            query, &second_dispatches, ws, config, llm, state, emitter,
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
        return AnalyzeOutcome::Proceed { dispatches: Vec::new(), llm_calls };
    }

    state.analyze_done = true;
    AnalyzeOutcome::Proceed { dispatches, llm_calls }
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

        warn!(retry = retries, "Cross-doc evidence insufficient, supplementing");
        retries += 1;

        let max_dispatch =
            MAX_SUPPLEMENTAL_DISPATCH.min(ws.doc_count() - state.dispatched.len());
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

/// Phase 4: Synthesize the final answer from collected evidence.
///
/// For single user-specified doc: uses simple `answer_synthesis` prompt.
/// For multi-doc or workspace: uses `orchestrator_integration` prompt.
///
/// Returns `(answer, llm_calls)`.
async fn synthesize(
    query: &str,
    ws: &WorkspaceContext<'_>,
    config: &Config,
    llm: &LlmClient,
    state: &OrchestratorState,
    emitter: &EventEmitter,
    skip_analysis: bool,
) -> (String, u32) {
    // Quality filter: drop SubAgent results with no meaningful evidence
    const MIN_EVIDENCE_CHARS: usize = 50;
    let quality_filtered: Vec<&Output> = state
        .sub_results
        .iter()
        .filter(|result| {
            if result.evidence.is_empty() {
                return false;
            }
            result
                .evidence
                .iter()
                .any(|e| e.content.len() >= MIN_EVIDENCE_CHARS)
        })
        .collect();

    let filtered_count = state.sub_results.len() - quality_filtered.len();
    if filtered_count > 0 {
        info!(
            filtered = filtered_count,
            kept = quality_filtered.len(),
            "Filtered low-quality SubAgent results"
        );
    }

    if !config.enable_synthesis || quality_filtered.is_empty() {
        return (format_evidence_as_answer(&state.all_evidence), 0);
    }

    // Single user-specified doc: simple synthesis
    if skip_analysis && ws.doc_count() == 1 {
        let evidence_text = format_evidence_for_synthesis(&state.all_evidence);
        let (system, user) = answer_synthesis(&SynthesisParams {
            query,
            evidence_text: &evidence_text,
            missing_info: "",
        });
        return match llm.complete(&system, &user).await {
            Ok(a) => {
                info!(answer_len = a.len(), "Synthesis complete");
                emitter.emit_synthesis(a.len());
                (a.trim().to_string(), 1)
            }
            Err(e) => {
                warn!(error = %e, "Synthesis LLM call failed");
                (format_evidence_as_answer(&state.all_evidence), 0)
            }
        };
    }

    // Multi-doc or workspace: orchestrator integration
    struct SubResultData {
        doc_name: String,
        evidence_count: usize,
        evidence_text: String,
        answer: String,
    }
    let summaries: Vec<SubResultData> = quality_filtered
        .iter()
        .map(|result| {
            let doc_name = result
                .evidence
                .first()
                .and_then(|e| e.doc_name.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let evidence_text = result
                .evidence
                .iter()
                .map(|e| format!("[{}] {}", e.node_title, e.content))
                .collect::<Vec<_>>()
                .join("\n");
            SubResultData {
                evidence_count: result.evidence.len(),
                doc_name,
                evidence_text,
                answer: result.answer.clone(),
            }
        })
        .collect();

    let summary_refs: Vec<super::prompts::SubAgentSummary<'_>> = summaries
        .iter()
        .map(|s| super::prompts::SubAgentSummary {
            doc_name: &s.doc_name,
            evidence_count: s.evidence_count,
            evidence_text: &s.evidence_text,
            answer: &s.answer,
        })
        .collect();

    let (system, user) = orchestrator_integration(&OrchestratorIntegrationParams {
        query,
        sub_results: &summary_refs,
    });

    match llm.complete(&system, &user).await {
        Ok(a) => {
            info!(answer_len = a.len(), "Synthesis complete");
            emitter.emit_synthesis(a.len());
            (a.trim().to_string(), 1)
        }
        Err(e) => {
            warn!(error = %e, "Orchestrator synthesis LLM call failed");
            (format_evidence_as_answer(&state.all_evidence), 0)
        }
    }
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

/// Maximum total characters for evidence in the orchestrator synthesis prompt.
const ORCH_SYNTHESIS_EVIDENCE_CAP: usize = 10000;

/// Format all evidence for the synthesis prompt, with a total character cap.
fn format_evidence_for_synthesis(evidence: &[super::config::Evidence]) -> String {
    let mut result = String::new();
    for e in evidence {
        let doc = e.doc_name.as_deref().unwrap_or("unknown");
        let item = format!(
            "[{}] ({} at {})\n{}",
            e.node_title, doc, e.source_path, e.content
        );
        if result.len() + item.len() + 2 > ORCH_SYNTHESIS_EVIDENCE_CAP {
            let remaining = ORCH_SYNTHESIS_EVIDENCE_CAP.saturating_sub(result.len());
            if remaining > 50 {
                result.push_str(&format!(
                    "[{}] ({} at {})\n{}...[truncated]\n",
                    e.node_title,
                    doc,
                    e.source_path,
                    &e.content[..remaining.min(e.content.len())]
                ));
            }
            let remaining_count = evidence.len()
                - evidence
                    .iter()
                    .position(|x| x.node_title == e.node_title)
                    .unwrap_or(0)
                - 1;
            if remaining_count > 0 {
                result.push_str(&format!(
                    "\n... and {} more evidence items truncated to fit budget.\n",
                    remaining_count
                ));
            }
            break;
        }
        result.push_str(&item);
        result.push_str("\n\n");
    }
    result
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

    // Simple synthesis
    let evidence_text = format_evidence_for_synthesis(&state.all_evidence);
    let (sys, usr) = answer_synthesis(&SynthesisParams {
        query,
        evidence_text: &evidence_text,
        missing_info: "",
    });

    let answer = match llm.complete(&sys, &usr).await {
        Ok(a) => {
            emitter.emit_synthesis(a.len());
            a.trim().to_string()
        }
        Err(_) => format_evidence_as_answer(&state.all_evidence),
    };

    let output = state.into_output(answer);
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

/// Format evidence as a simple answer (fallback).
fn format_evidence_as_answer(evidence: &[super::config::Evidence]) -> String {
    evidence
        .iter()
        .map(|e| {
            let doc = e.doc_name.as_deref().unwrap_or("unknown");
            format!(
                "**{}** (from {} at {}):\n{}",
                e.node_title, doc, e.source_path, e.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
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
