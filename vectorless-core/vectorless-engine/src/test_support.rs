// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Test-only helpers for constructing Engine instances without a real LLM.
//!
//! This module is exposed via `vectorless::__test_support` and should **only**
//! be used in integration tests.

use std::sync::Arc;

use crate::engine::Engine;
use crate::indexer::IndexerClient;
use crate::retriever::RetrieverClient;
use vectorless_config::Config;
use vectorless_events::EventEmitter;
use vectorless_index::PipelineExecutor;
use vectorless_llm::LlmClient;
use vectorless_llm::config::LlmConfig;
use vectorless_metrics::MetricsHub;
use vectorless_storage::Workspace;

/// Build an `Engine` with a no-LLM pipeline for integration testing.
///
/// The pipeline skips enhance/summary stages but exercises:
/// parse → build → validate → split → enrich → optimize.
///
/// # Example
///
/// ```rust,ignore
/// let tmp = tempfile::tempdir().unwrap();
/// let engine = vectorless::__test_support::build_test_engine(tmp.path()).await;
/// ```
pub async fn build_test_engine(workspace_dir: &std::path::Path) -> Engine {
    let config = Config::default();

    // No-LLM indexer: pipeline without enhance stage
    let executor_factory: Arc<dyn Fn() -> PipelineExecutor + Send + Sync> =
        Arc::new(|| PipelineExecutor::new());
    let indexer = IndexerClient::with_factory(executor_factory);

    let workspace = Workspace::new(workspace_dir).await.unwrap();
    let retriever = RetrieverClient::new(LlmClient::new(LlmConfig::default()));

    Engine::with_components(
        config,
        workspace,
        retriever,
        indexer,
        EventEmitter::new(),
        Arc::new(MetricsHub::with_defaults()),
    )
    .await
    .unwrap()
}
