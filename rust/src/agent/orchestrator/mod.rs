// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Orchestrator agent — supervisor loop for multi-document retrieval.
//!
//! The Orchestrator is a consuming-self struct implementing [`Agent`]:
//! 1. Analyze: LLM selects documents + tasks (informed by QueryPlan)
//! 2. Supervisor loop: dispatch → evaluate → replan if insufficient
//! 3. Rerank: dedup → BM25 scoring → synthesis/fusion

mod analyze;
mod dispatch;
mod evaluate;
mod replan;

use tracing::info;

use crate::llm::LlmClient;
use crate::query::QueryPlan;

use super::Agent;
use super::config::{AgentConfig, Output, WorkspaceContext};
use super::events::EventEmitter;
use super::state::OrchestratorState;
use super::tools::orchestrator as orch_tools;

use analyze::{AnalyzeOutcome, analyze};
use evaluate::evaluate;
use replan::replan;

/// Maximum supervisor loop iterations to prevent infinite loops.
const MAX_SUPERVISOR_ITERATIONS: u32 = 3;

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
        let Orchestrator {
            query,
            ws,
            config,
            llm,
            emitter,
            skip_analysis,
            query_plan,
        } = self;

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

        // --- Phase 1: Analyze — LLM selects documents + tasks ---
        let initial_dispatches = match analyze(
            &query,
            ws,
            &mut state,
            &emitter,
            skip_analysis,
            &query_plan,
            &llm,
        )
        .await?
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
                emitter.emit_orchestrator_completed(0, orch_llm_calls + llm_calls, 0);
                return Ok(output);
            }
            AnalyzeOutcome::NoResults { llm_calls } => {
                emitter.emit_orchestrator_completed(0, orch_llm_calls + llm_calls, 0);
                return Ok(Output::empty());
            }
        };

        // --- Phase 2: Supervisor loop ---
        let mut current_dispatches = initial_dispatches;
        let mut iteration: u32 = 0;
        let mut eval_sufficient = false;

        loop {
            if iteration >= MAX_SUPERVISOR_ITERATIONS {
                info!(iteration, "Supervisor loop budget exhausted");
                break;
            }

            // Dispatch current plan
            if !current_dispatches.is_empty() {
                info!(
                    docs = current_dispatches.len(),
                    docs_list = ?current_dispatches.iter().map(|d| d.doc_idx).collect::<Vec<_>>(),
                    iteration,
                    "Dispatching Workers"
                );
                dispatch::dispatch_and_collect(
                    &query,
                    &current_dispatches,
                    ws,
                    &config,
                    &llm,
                    &mut state,
                    &emitter,
                    &query_plan,
                )
                .await;
            }

            // No evidence at all — nothing to evaluate
            if state.all_evidence.is_empty() {
                info!("No evidence collected from any Worker");
                break;
            }

            // Skip evaluation for user-specified documents (no replan needed)
            if skip_analysis {
                break;
            }

            // Evaluate sufficiency
            let eval_result = evaluate(&query, &state.all_evidence, &llm).await?;
            orch_llm_calls += 1;

            if eval_result.sufficient {
                eval_sufficient = true;
                info!(
                    evidence = state.all_evidence.len(),
                    iteration, "Evidence sufficient — exiting supervisor loop"
                );
                break;
            }

            // Insufficient — replan
            info!(
                evidence = state.all_evidence.len(),
                missing = eval_result.missing_info.len(),
                iteration,
                "Evidence insufficient — replanning"
            );

            let doc_cards_text = orch_tools::ls_docs(ws).feedback;
            let replan_result = replan(
                &query,
                &eval_result.missing_info,
                &state.all_evidence,
                &state.dispatched,
                ws.doc_count(),
                &doc_cards_text,
                &llm,
            )
            .await?;
            orch_llm_calls += 1;

            if replan_result.dispatches.is_empty() {
                info!("Replan produced no new dispatches — exiting supervisor loop");
                break;
            }

            current_dispatches = replan_result.dispatches;
            iteration += 1;
        }

        // Derive confidence from supervisor loop outcome:
        // - LLM evaluated sufficient on first try → high confidence
        // - Needed replan rounds → lower confidence
        // - No evaluation ran (skip_analysis / no evidence) → moderate
        let confidence =
            compute_confidence(eval_sufficient, iteration, state.all_evidence.is_empty());

        // --- Phase 3: Finalize — rerank + synthesize ---
        if state.all_evidence.is_empty() {
            emitter.emit_orchestrator_completed(0, orch_llm_calls, 0);
            return Ok(state.into_output(String::new()));
        }

        let multi_doc = ws.doc_count() > 1;
        finalize_output(
            &query,
            &state,
            &config,
            &llm,
            &emitter,
            orch_llm_calls,
            multi_doc,
            query_plan.intent,
            confidence,
        )
        .await
    }
}

/// Compute confidence from LLM evaluate() outcome.
fn compute_confidence(eval_sufficient: bool, replan_rounds: u32, no_evidence: bool) -> f32 {
    if no_evidence {
        return 0.0;
    }
    if eval_sufficient {
        // LLM said sufficient: first round = 0.95, each replan round drops 0.15
        (0.95 - replan_rounds as f32 * 0.15).max(0.5)
    } else {
        // LLM never said sufficient (budget exhausted or no more docs)
        (0.4 - replan_rounds as f32 * 0.1).max(0.1)
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
    intent: crate::query::QueryIntent,
    confidence: f32,
) -> crate::error::Result<Output> {
    let _ = config;
    let rerank_result = crate::rerank::process(
        query,
        &state.all_evidence,
        llm,
        multi_doc,
        &state.sub_results,
        intent,
        confidence,
    )
    .await?;

    let total_llm_calls = orch_llm_calls + rerank_result.llm_calls;
    if !rerank_result.answer.is_empty() {
        emitter.emit_answer_completed(rerank_result.answer.len(), "medium");
    }

    let mut output = state.clone_results_into_output(rerank_result.answer);
    output.metrics.llm_calls += total_llm_calls;
    output.confidence = rerank_result.confidence;

    emitter.emit_orchestrator_completed(
        output.evidence.len(),
        output.metrics.llm_calls,
        output.metrics.rounds_used,
    );

    info!(
        evidence = output.evidence.len(),
        llm_calls = output.metrics.llm_calls,
        confidence = output.confidence,
        "Orchestrator complete"
    );

    Ok(output)
}
