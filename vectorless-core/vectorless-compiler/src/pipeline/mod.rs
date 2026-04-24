// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pipeline execution module.
//!
//! This module provides the core pipeline infrastructure:
//! - [`CompileContext`] - Context passed between passes
//! - [`PipelineExecutor`] - Executes the compilation pipeline
//! - [`PipelineOrchestrator`] - Flexible pass orchestration with dependencies
//! - [`CompileMetrics`] - Performance metrics collection
//! - [`FailurePolicy`] - Configurable failure handling for passes
//! - [`StageRetryConfig`] - Retry configuration for passes

mod checkpoint;
mod context;
mod executor;
mod metrics;
mod orchestrator;
mod policy;

pub use context::{CompileContext, CompileResult, CompilerInput, PassResult};
pub use executor::PipelineExecutor;
pub use metrics::CompileMetrics;
pub use policy::{FailurePolicy, StageRetryConfig};
