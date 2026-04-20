// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Orchestrator loop — multi-document retrieval via MapReduce.
//!
//! Flow:
//! 1. Fast path: find_cross → direct hit across all docs
//! 2. Analyze: ls_docs + find_cross → LLM decides which docs + tasks
//! 3. Dispatch: fan-out N Workers in parallel
//! 4. Integrate: merge evidence, check cross-doc sufficiency, optionally re-dispatch
//! 5. Rerank: dedup → BM25 scoring → synthesis/fusion

mod analyze;
mod dispatch;
mod fast_path;
mod integrate;

use tracing::info;

use crate::llm::LlmClient;

use super::config::{Config, Output, WorkspaceContext};
use super::events::EventEmitter;
use super::state::OrchestratorState;

use analyze::{AnalyzeOutcome, analyze};
use dispatch::fallback_dispatch_all;
use integrate::integrate;

/// Run the Orchestrator loop for multi-document retrieval.
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
        if let Some(output) = fast_path::fast_path(query, ws, config, emitter) {
            info!("Orchestrator fast path hit — skipping dispatch");
            emitter.emit_completed(
                output.evidence.len(), output.metrics.llm_calls,
                output.metrics.rounds_used, true, false, false, 0,
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
            "Phase 2: dispatching Workers"
        );
        dispatch::dispatch_and_collect(query, &dispatches, ws, config, llm, &mut state, emitter).await;
    }

    // --- Phase 3: Integrate ---
    if state.all_evidence.is_empty() {
        info!("No evidence collected from any Worker");
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
    finalize_output(query, &state, config, llm, emitter, orch_llm_calls, multi_doc).await
}

/// Rerank evidence and emit completion events.
///
/// Shared by `run()` and `fallback_dispatch_all()` to avoid duplication.
pub async fn finalize_output(
    query: &str,
    state: &OrchestratorState,
    config: &Config,
    llm: &LlmClient,
    emitter: &EventEmitter,
    orch_llm_calls: u32,
    multi_doc: bool,
) -> crate::error::Result<Output> {
    let rerank_result = crate::rerank::process(
        query, &state.all_evidence, config, llm, multi_doc, &state.sub_results,
    )
    .await;

    let total_llm_calls = orch_llm_calls + rerank_result.llm_calls;
    if !rerank_result.answer.is_empty() {
        emitter.emit_synthesis(rerank_result.answer.len());
    }

    let mut output = state.clone_results_into_output(rerank_result.answer);
    output.metrics.llm_calls += total_llm_calls;
    output.score = rerank_result.score;

    emitter.emit_completed(
        output.evidence.len(), output.metrics.llm_calls,
        output.metrics.rounds_used, output.metrics.fast_path_hit,
        output.metrics.budget_exhausted, output.metrics.plan_generated,
        output.metrics.evidence_chars,
    );

    info!(
        evidence = output.evidence.len(),
        llm_calls = output.metrics.llm_calls,
        "Orchestrator complete"
    );

    Ok(output)
}
