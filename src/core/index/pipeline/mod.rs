// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pipeline execution module.
//!
//! This module provides the core pipeline infrastructure:
//! - [`IndexContext`] - Context passed between stages
//! - [`PipelineExecutor`] - Executes the indexing pipeline
//! - [`PipelineOrchestrator`] - Flexible stage orchestration with dependencies
//! - [`IndexMetrics`] - Performance metrics collection

mod context;
mod executor;
mod metrics;
mod orchestrator;

pub use context::{IndexContext, IndexInput, IndexResult, StageResult};
pub use executor::PipelineExecutor;
pub use metrics::IndexMetrics;
pub use orchestrator::{PipelineOrchestrator, CustomStageBuilder};
