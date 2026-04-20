// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Phase 2: Dispatch Workers and collect results.

use tracing::{info, warn};

use crate::llm::LlmClient;

use super::super::config::{AgentConfig, Output, WorkspaceContext};
use super::super::events::EventEmitter;
use super::super::prompts::DispatchEntry;
use super::super::state::OrchestratorState;
use super::super::worker::Worker;
use super::super::Agent;

/// Dispatch Workers in parallel and collect results.
pub async fn dispatch_and_collect(
    query: &str,
    dispatches: &[DispatchEntry],
    ws: &WorkspaceContext<'_>,
    config: &AgentConfig,
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

            let query = query.to_string();
            let task = dispatch.task.clone();
            let worker_config = config.worker.clone();
            let doc_idx = dispatch.doc_idx;
            let doc_name = doc.doc_name.to_string();
            let llm = llm.clone();
            let sub_emitter = EventEmitter::noop();

            Some(async move {
                emitter.emit_worker_dispatched(doc_idx, &doc_name, &task, &[]);
                let worker = Worker::new(
                    &query, Some(&task), doc, worker_config, llm, sub_emitter,
                );
                let result = worker.run().await;
                (doc_idx, doc_name, result)
            })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(futures).await;

    for (doc_idx, doc_name, result) in results {
        match result {
            Ok(output) => {
                info!(doc_idx, evidence = output.evidence.len(), "Worker completed");
                emitter.emit_worker_completed(
                    doc_idx, &doc_name,
                    output.evidence.len(),
                    output.metrics.rounds_used,
                    output.metrics.llm_calls,
                    true,
                );
                state.collect_result(doc_idx, output);
            }
            Err(e) => {
                warn!(doc_idx, error = %e, "Worker failed");
                emitter.emit_worker_completed(doc_idx, &doc_name, 0, 0, 0, false);
            }
        }
    }
}

/// Fallback: dispatch Workers to all documents with the original query.
pub async fn fallback_dispatch_all(
    query: &str,
    ws: &WorkspaceContext<'_>,
    config: &AgentConfig,
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
        emitter.emit_orchestrator_completed(0, 0, 0);
        return Ok(state.into_output(String::new()));
    }

    let multi_doc = ws.doc_count() > 1;
    super::finalize_output(query, &state, config, llm, emitter, 0, multi_doc).await
}
