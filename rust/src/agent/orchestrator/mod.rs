// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Orchestrator agent — multi-document retrieval via MapReduce.
//!
//! The Orchestrator is a consuming-self struct implementing [`Agent`]:
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
use crate::query::QueryPlan;

use super::config::{AgentConfig, Output, WorkspaceContext};
use super::events::EventEmitter;
use super::state::OrchestratorState;
use super::Agent;

use analyze::{AnalyzeOutcome, analyze};
use integrate::integrate;

/// Orchestrator agent — coordinates multi-document retrieval.
///
/// Holds all execution context. Calling [`run()`](Agent::run) consumes self.
pub struct Orchestrator<'a> {
    query: String,
    ws: &'a WorkspaceContext<'a>,
    config: AgentConfig,
    llm: LlmClient,
    emitter: EventEmitter,
    skip_analysis: bool,
    /// Query understanding plan — produced by `QueryPipeline::understand()`.
    /// Contains intent, complexity, key concepts, and strategy hints.
    query_plan: QueryPlan,
}

impl<'a> Orchestrator<'a> {
    /// Create a new Orchestrator.
    pub fn new(
        query: &str,
        ws: &'a WorkspaceContext<'a>,
        config: AgentConfig,
        llm: LlmClient,
        emitter: EventEmitter,
        skip_analysis: bool,
        query_plan: QueryPlan,
    ) -> Self {
        Self {
            query: query.to_string(),
            ws,
            config,
            llm,
            emitter,
            skip_analysis,
            query_plan,
        }
    }
}

impl<'a> Agent for Orchestrator<'a> {
    type Output = Output;

    fn name(&self) -> &str {
        "orchestrator"
    }

    async fn run(self) -> crate::error::Result<Output> {
        let Orchestrator { query, ws, config, llm, emitter, skip_analysis, query_plan } = self;

        info!(
            docs = ws.doc_count(),
            skip_analysis,
            intent = %query_plan.intent,
            complexity = %query_plan.complexity,
            "Orchestrator starting"
        );
        emitter.emit_orchestrator_started(&query, ws.doc_count(), skip_analysis);

        let mut state = OrchestratorState::new();
        let mut orch_llm_calls: u32 = 0;

        // --- Phase 0: Fast path ---
        if config.orchestrator.enable_fast_path {
            if let Some(output) = fast_path::fast_path(
                &query, ws, config.orchestrator.enable_fast_path,
                &config.orchestrator.worker_config.fast_path_threshold, &emitter,
            ) {
                info!("Orchestrator fast path hit — skipping dispatch");
                emitter.emit_orchestrator_completed(
                    output.evidence.len(), output.metrics.llm_calls,
                    output.metrics.rounds_used,
                );
                return Ok(output);
            }
        }

        // --- Phase 1: Analyze (uses query_plan for intent-aware strategy) ---
        let dispatches = match analyze(&query, ws, &mut state, &emitter, skip_analysis, &query_plan, &llm).await? {
            AnalyzeOutcome::Proceed { dispatches, llm_calls } => {
                orch_llm_calls += llm_calls;
                dispatches
            }
            AnalyzeOutcome::AlreadyAnswered { llm_calls } => {
                let mut output = Output::empty();
                output.answer = "Already answered by cross-document search.".to_string();
                emitter.emit_orchestrator_completed(0, orch_llm_calls + llm_calls, 0);
                return Ok(output);
            }
            AnalyzeOutcome::NoResults { llm_calls } => {
                emitter.emit_orchestrator_completed(0, orch_llm_calls + llm_calls, 0);
                return Ok(Output::empty());
            }
        };

        // --- Phase 2: Dispatch ---
        if !dispatches.is_empty() {
            info!(
                docs = dispatches.len(),
                docs_list = ?dispatches.iter().map(|d| d.doc_idx).collect::<Vec<_>>(),
                "Phase 2: dispatching Workers"
            );
            dispatch::dispatch_and_collect(&query, &dispatches, ws, &config, &llm, &mut state, &emitter).await;
        }

        // --- Phase 3: Integrate ---
        if state.all_evidence.is_empty() {
            info!("No evidence collected from any Worker");
            emitter.emit_orchestrator_completed(0, orch_llm_calls, 0);
            return Ok(state.into_output(
                "I was unable to find relevant information across the available documents to answer your question.".to_string()
            ));
        }

        if !skip_analysis {
            orch_llm_calls += integrate(&query, ws, &config, &llm, &mut state, &emitter).await;
        }

        // --- Phase 4: Rerank ---
        let multi_doc = !skip_analysis || ws.doc_count() > 1;
        finalize_output(&query, &state, &config, &llm, &emitter, orch_llm_calls, multi_doc).await
    }
}

/// Rerank evidence and emit completion events.
pub async fn finalize_output(
    query: &str,
    state: &OrchestratorState,
    config: &AgentConfig,
    llm: &LlmClient,
    emitter: &EventEmitter,
    orch_llm_calls: u32,
    multi_doc: bool,
) -> crate::error::Result<Output> {
    let rerank_result = crate::rerank::process(
        query, &state.all_evidence, config.answer.enable_synthesis, llm, multi_doc, &state.sub_results,
    )
    .await?;

    let total_llm_calls = orch_llm_calls + rerank_result.llm_calls;
    if !rerank_result.answer.is_empty() {
        emitter.emit_answer_completed(rerank_result.answer.len(), "medium");
    }

    let mut output = state.clone_results_into_output(rerank_result.answer);
    output.metrics.llm_calls += total_llm_calls;
    output.score = rerank_result.score;

    emitter.emit_orchestrator_completed(
        output.evidence.len(), output.metrics.llm_calls,
        output.metrics.rounds_used,
    );

    info!(
        evidence = output.evidence.len(),
        llm_calls = output.metrics.llm_calls,
        "Orchestrator complete"
    );

    Ok(output)
}
