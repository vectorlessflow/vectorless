// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval dispatcher — the single entry point for all query operations.
//!
//! All queries go through the Orchestrator. There is no separate SubAgent path.
//! The Orchestrator internally decides whether to run the full analysis phase
//! based on user intent:
//!
//! - **User specified doc_ids** → Orchestrator skips analysis, spawns N SubAgents
//!   directly (N=1 is a normal case, not special).
//! - **User unspecified (workspace)** → Orchestrator analyzes DocCards, selects
//!   relevant docs, then spawns SubAgents.
//!
//! Post-processing (synthesis, dedup, rerank) is always unified through the
//! Orchestrator's output — never duplicated in SubAgent.

use tracing::info;

use crate::agent::{Config, EventEmitter, Output, Scope, WorkspaceContext};
use crate::error::{Error, Result};
use crate::llm::LlmClient;

/// Dispatch a query to the Orchestrator.
///
/// This is the single entry point from the client layer into the retrieval system.
/// It always goes through the Orchestrator — never directly to SubAgent.
///
/// - `Scope::Specified(docs)` → Orchestrator skips analysis, dispatches all docs directly.
/// - `Scope::Workspace(ws)` → Orchestrator runs full flow (analyze → dispatch → fuse → synthesize).
pub async fn dispatch(
    query: &str,
    scope: Scope<'_>,
    config: &Config,
    llm: &LlmClient,
    emitter: &EventEmitter,
) -> Result<Output> {
    let (ws, skip_analysis) = match scope {
        Scope::Specified(docs) => {
            info!(docs = docs.len(), "Dispatch (user-specified, skip analysis)");
            (WorkspaceContext::new(docs), true)
        }
        Scope::Workspace(ws) => {
            info!(docs = ws.doc_count(), "Dispatch (workspace, full flow)");
            (ws, false)
        }
    };

    crate::agent::orchestrator::run(query, &ws, config, llm, emitter, skip_analysis)
        .await
        .map_err(|e| Error::Retrieval(e.to_string()))
}
