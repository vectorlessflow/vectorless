// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Phase 1: Analyze documents and produce a dispatch plan.
//!
//! Uses the [`QueryPlan`] from query understanding to inform document selection.
//! LLM errors propagate — no silent degradation.

use tracing::{debug, info};

use vectorless_error::Error;
use vectorless_llm::LlmClient;
use vectorless_query::QueryPlan;
use vectorless_scoring::bm25::extract_keywords;

use super::super::config::WorkspaceContext;
use super::super::prompts::{DispatchEntry, orchestrator_analysis, parse_dispatch_plan};
use super::super::state::OrchestratorState;
use super::super::tools::orchestrator as orch_tools;

/// Outcome of the analyze phase.
pub enum AnalyzeOutcome {
    /// Produce dispatch entries for Phase 2.
    Proceed {
        dispatches: Vec<DispatchEntry>,
        llm_calls: u32,
    },
    /// Cross-doc search already answered the query.
    AlreadyAnswered { llm_calls: u32 },
    /// No relevant documents found.
    NoResults { llm_calls: u32 },
}

/// Analyze documents and produce a dispatch plan.
///
/// Uses the [`QueryPlan`] for intent-aware analysis:
/// - Intent and key concepts inform the LLM about what to look for
/// - Complexity hints at how many documents may be needed
/// - Strategy hint guides the analysis approach
///
/// LLM failures propagate as [`Error::LlmReasoning`] — no fallback.
pub async fn analyze(
    query: &str,
    ws: &WorkspaceContext<'_>,
    state: &mut OrchestratorState,
    emitter: &crate::EventEmitter,
    skip_analysis: bool,
    query_plan: &QueryPlan,
    llm: &LlmClient,
) -> vectorless_error::Result<AnalyzeOutcome> {
    if skip_analysis {
        debug!("Phase 1: skipping (user-specified documents)");
        let dispatches = (0..ws.doc_count())
            .map(|idx| DispatchEntry {
                doc_idx: idx,
                reason: "User-specified document".to_string(),
                task: query.to_string(),
            })
            .collect();
        return Ok(AnalyzeOutcome::Proceed {
            dispatches,
            llm_calls: 0,
        });
    }

    debug!(
        intent = %query_plan.intent,
        complexity = %query_plan.complexity,
        strategy = query_plan.strategy_hint,
        "Phase 1: analyzing doc cards with query understanding"
    );

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

    // Build analysis prompt enriched with query understanding
    let concepts_text = if query_plan.key_concepts.is_empty() {
        String::new()
    } else {
        format!("\nKey concepts: {}", query_plan.key_concepts.join(", "))
    };

    let strategy_text = if query_plan.strategy_hint.is_empty() {
        String::new()
    } else {
        format!("\nRetrieval strategy: {}", query_plan.strategy_hint)
    };

    let rewritten_text = if query_plan.rewritten.is_empty() {
        String::new()
    } else {
        format!(
            "\nRewritten queries for matching: {}",
            query_plan.rewritten.join("; ")
        )
    };

    let intent_context = format!(
        "\nQuery intent: {} (complexity: {}){concepts_text}{strategy_text}{rewritten_text}",
        query_plan.intent, query_plan.complexity,
    );

    let (system, user) =
        orchestrator_analysis(&super::super::prompts::OrchestratorAnalysisParams {
            query,
            doc_cards: &doc_cards_text,
            find_results: &find_text,
            intent_context: &intent_context,
        });

    let analysis_output = llm.complete(&system, &user).await.map_err(|e| {
        emitter.emit_error("orchestrator/analysis", &e.to_string());
        Error::LlmReasoning {
            stage: "orchestrator/analysis".to_string(),
            detail: format!("LLM call failed: {e}"),
        }
    })?;

    info!(
        response_len = analysis_output.len(),
        response = %if analysis_output.len() > 500 { &analysis_output[..500] } else { &analysis_output },
        "Phase 1: analysis LLM response"
    );

    let dispatches = match parse_dispatch_plan(&analysis_output, ws.doc_count()) {
        Some(entries) => entries,
        None => {
            info!("Orchestrator: analysis indicates already answered");
            return Ok(AnalyzeOutcome::AlreadyAnswered { llm_calls: 1 });
        }
    };

    info!(
        dispatches = dispatches.len(),
        "Phase 1: parsed dispatch plan"
    );

    if dispatches.is_empty() {
        return Ok(AnalyzeOutcome::NoResults { llm_calls: 1 });
    }

    state.analyze_done = true;
    Ok(AnalyzeOutcome::Proceed {
        dispatches,
        llm_calls: 1,
    })
}
