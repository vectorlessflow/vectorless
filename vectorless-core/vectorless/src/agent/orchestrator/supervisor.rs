// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Phase 2: Supervisor loop — dispatch → evaluate → replan.

use tracing::info;

use crate::llm::LlmClient;
use crate::query::QueryPlan;

use super::super::config::{AgentConfig, WorkspaceContext};
use super::super::events::EventEmitter;
use super::super::prompts::DispatchEntry;
use super::super::state::OrchestratorState;
use super::super::tools::orchestrator as orch_tools;
use super::MAX_SUPERVISOR_ITERATIONS;
use super::dispatch;
use super::evaluate::evaluate;
use super::replan::replan;

/// Outcome of the supervisor loop.
pub struct SupervisorOutcome {
    /// Number of replan iterations performed.
    pub iteration: u32,
    /// Whether the LLM evaluator judged evidence sufficient.
    pub eval_sufficient: bool,
    /// LLM calls consumed within the supervisor loop itself.
    pub llm_calls: u32,
}

/// Run the supervisor loop: dispatch → evaluate → replan.
///
/// Returns a [`SupervisorOutcome`] summarizing what happened.
pub async fn run_supervisor_loop(
    query: &str,
    initial_dispatches: Vec<DispatchEntry>,
    ws: &WorkspaceContext<'_>,
    config: &AgentConfig,
    llm: &LlmClient,
    state: &mut OrchestratorState,
    emitter: &EventEmitter,
    query_plan: &QueryPlan,
    skip_analysis: bool,
) -> crate::error::Result<SupervisorOutcome> {
    let mut current_dispatches = initial_dispatches;
    let mut iteration: u32 = 0;
    let mut eval_sufficient = false;
    let mut llm_calls: u32 = 0;

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
                query,
                &current_dispatches,
                ws,
                config,
                llm,
                state,
                emitter,
                query_plan,
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
            eval_sufficient = !state.all_evidence.is_empty();
            break;
        }

        // Evaluate sufficiency
        let eval_result = evaluate(query, &state.all_evidence, llm).await?;
        llm_calls += 1;

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
            query,
            &eval_result.missing_info,
            &state.all_evidence,
            &state.dispatched,
            ws.doc_count(),
            &doc_cards_text,
            llm,
        )
        .await?;
        llm_calls += 1;

        if replan_result.dispatches.is_empty() {
            info!("Replan produced no new dispatches — exiting supervisor loop");
            break;
        }

        current_dispatches = replan_result.dispatches;
        iteration += 1;
    }

    Ok(SupervisorOutcome {
        iteration,
        eval_sufficient,
        llm_calls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supervisor_outcome_fields() {
        let outcome = SupervisorOutcome {
            iteration: 2,
            eval_sufficient: true,
            llm_calls: 5,
        };
        assert_eq!(outcome.iteration, 2);
        assert!(outcome.eval_sufficient);
        assert_eq!(outcome.llm_calls, 5);
    }

    #[test]
    fn test_max_iterations_constant() {
        assert_eq!(MAX_SUPERVISOR_ITERATIONS, 3);
    }
}
