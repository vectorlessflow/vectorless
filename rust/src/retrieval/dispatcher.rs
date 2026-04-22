// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval dispatcher — the single entry point for all query operations.
//!
//! All queries go through the Orchestrator. There is no separate Worker path.
//! The Orchestrator internally decides whether to run the full analysis phase
//! based on user intent:
//!
//! - **User specified doc_ids** → Orchestrator skips analysis, spawns N Workers
//!   directly (N=1 is a normal case, not special).
//! - **User unspecified (workspace)** → Orchestrator analyzes DocCards, selects
//!   relevant docs, then spawns Workers.
//!
//! Post-processing (synthesis, dedup, rerank) is always unified through the
//! Orchestrator's output — never duplicated in Worker.

use tracing::info;

use crate::agent::config::{AgentConfig, Scope, WorkspaceContext};
use crate::agent::orchestrator::Orchestrator;
use crate::agent::{Agent, EventEmitter, Output};
use crate::error::{Error, Result};
use crate::llm::LlmClient;
use crate::query::QueryPipeline;

/// Dispatch a query to the Orchestrator.
///
/// This is the single entry point from the client layer into the retrieval system.
/// It always goes through the Orchestrator — never directly to Worker.
///
/// Flow:
/// 1. Query understanding via LLM (produces [`QueryPlan`])
/// 2. Orchestrator dispatch (uses QueryPlan for strategy)
///
/// - `Scope::Specified(docs)` → Orchestrator skips analysis, dispatches all docs directly.
/// - `Scope::Workspace(ws)` → Orchestrator runs full flow (analyze → dispatch → fuse → synthesize).
pub async fn dispatch(
    query: &str,
    scope: Scope<'_>,
    config: &AgentConfig,
    llm: &LlmClient,
    emitter: &EventEmitter,
) -> Result<Output> {
    let (ws, skip_analysis) = match scope {
        Scope::Specified(docs) => {
            info!(
                docs = docs.len(),
                "Dispatch (user-specified, skip analysis)"
            );
            (WorkspaceContext::new(docs), true)
        }
        Scope::Workspace(ws) => {
            info!(docs = ws.doc_count(), "Dispatch (workspace, full flow)");
            (ws, false)
        }
    };

    // Step 1: Query understanding — LLM analyzes intent, concepts, complexity.
    // This is required. "Model fails, we fail." — errors propagate.
    info!("Starting query understanding...");
    let query_plan = QueryPipeline::understand(query, llm).await?;

    // Step 2: Dispatch to Orchestrator with the query plan.
    let orchestrator = Orchestrator::new(
        query,
        &ws,
        config.clone(),
        llm.clone(),
        emitter.clone(),
        skip_analysis,
        query_plan,
    );
    orchestrator
        .run()
        .await
        .map_err(|e| Error::Retrieval(e.to_string()))
}
