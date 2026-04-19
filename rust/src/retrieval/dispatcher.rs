// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval dispatcher — the entry point for all query operations.
//!
//! Decides the execution path based on user intent:
//!
//! - **User specified doc_ids** → parallel spawn N × SubAgent (N=1 is a special case)
//! - **User unspecified (workspace)** → Orchestrator analyzes DocCards, then spawns SubAgents

use tracing::info;
use futures::StreamExt;

use crate::agent::{self, Config, DocContext, EventEmitter, Output, Scope};
use crate::error::{Error, Result};
use crate::llm::LlmClient;

/// Dispatch a query to the appropriate agent path.
///
/// This is the single entry point from the client layer into the retrieval system.
/// It replaces the old `agent::retrieve()` routing function.
pub async fn dispatch(
    query: &str,
    scope: Scope<'_>,
    config: &Config,
    llm: &LlmClient,
    emitter: &EventEmitter,
) -> Result<Output> {
    match &scope {
        // User specified documents → SubAgent directly (no Orchestrator analysis needed)
        Scope::Single(_) => {
            let doc_ctx = match &scope {
                Scope::Single(ctx) => ctx,
                Scope::Workspace(_) => unreachable!(),
            };
            info!(doc = doc_ctx.doc_name, "Dispatching to SubAgent (user-specified document)");
            agent::subagent::run(query, None, doc_ctx, config, llm, emitter)
                .await
                .map_err(|e| Error::Retrieval(e.to_string()))
        }

        // Workspace scope → Orchestrator analyzes and dispatches
        Scope::Workspace(ws_ctx) => {
            info!(
                docs = ws_ctx.docs.len(),
                "Dispatching to Orchestrator (workspace scope)"
            );
            agent::orchestrator::run(query, ws_ctx, config, llm, emitter)
                .await
                .map_err(|e| Error::Retrieval(e.to_string()))
        }
    }
}

/// Dispatch a query across multiple user-specified documents in parallel.
///
/// Each document gets its own SubAgent. This is used when the user explicitly
/// specifies which documents to query (doc_ids), regardless of count.
pub async fn dispatch_parallel(
    query: &str,
    doc_contexts: Vec<DocContext<'_>>,
    config: &Config,
    llm: &LlmClient,
    emitter: &EventEmitter,
) -> Vec<(String, Result<Output>)> {
    let concurrency = 4; // TODO: make configurable
    let results: Vec<(String, Result<Output>)> = futures::stream::iter(doc_contexts.into_iter())
        .map(|doc_ctx| {
            let query = query.to_string();
            let config = config.clone();
            let llm = llm.clone();
            let emitter = emitter.clone();
            async move {
                let doc_name = doc_ctx.doc_name.to_string();
                let result = agent::subagent::run(
                    &query,
                    None,
                    &doc_ctx,
                    &config,
                    &llm,
                    &emitter,
                )
                .await
                .map_err(|e| Error::Retrieval(e.to_string()));
                (doc_name, result)
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;

    results
}
