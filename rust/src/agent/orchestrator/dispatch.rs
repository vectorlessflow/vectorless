// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Phase 2: Dispatch SubAgents and collect results.

use tracing::{info, warn};

use crate::llm::LlmClient;

use super::super::config::{Config, Output, WorkspaceContext};
use super::super::events::EventEmitter;
use super::super::prompts::DispatchEntry;
use super::super::state::OrchestratorState;
use super::super::subagent;

/// Dispatch SubAgents in parallel and collect results.
pub async fn dispatch_and_collect(
    query: &str,
    dispatches: &[DispatchEntry],
    ws: &WorkspaceContext<'_>,
    config: &Config,
    llm: &LlmClient,
    state: &mut OrchestratorState,
    emitter: &EventEmitter,
) {
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
            let llm = llm.clone();
            let sub_emitter = EventEmitter::noop();

            Some(async move {
                emitter.emit_subagent_dispatched(doc_idx, &doc_name, &task);
                let result =
                    subagent::run(&query, Some(&task), doc, &config, &llm, &sub_emitter).await;
                (doc_idx, result)
            })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(futures).await;

    for (doc_idx, result) in results {
        match result {
            Ok(output) => {
                info!(doc_idx, evidence = output.evidence.len(), "SubAgent completed");
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

/// Fallback: dispatch SubAgents to all documents with the original query.
pub async fn fallback_dispatch_all(
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

    let multi_doc = ws.doc_count() > 1;
    super::finalize_output(query, &state, config, llm, emitter, 0, multi_doc).await
}
