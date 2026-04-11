// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval pipeline infrastructure.
//!
//! This module provides the core pipeline infrastructure for retrieval:
//! - [`RetrievalStage`] - Trait for pipeline stages
//! - [`PipelineContext`] - Context passed between stages
//! - [`StageOutcome`] - Controls pipeline flow (continue, backtrack, etc.)
//! - [`RetrievalOrchestrator`] - Manages stage execution
//!
//!
//! # Flow Control
//!
//! Unlike the index pipeline, retrieval stages can control flow:
//!
//! - `Continue` - Proceed to next stage
//! - `Complete` - Retrieval is done, return results
//! - `NeedMoreData` - Backtrack to search for more data
//! - `Backtrack` - Return to a specific stage
//! - `Skip` - Skip remaining stages
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::retrieval::pipeline::{RetrievalOrchestrator, RetrievalStage};
//!
//! let orchestrator = RetrievalOrchestrator::new()
//!     .stage(AnalyzeStage::new())
//!     .stage(PlanStage::new())
//!     .stage(SearchStage::new())
//!     .stage(EvaluateStage::new());
//!
//! let response = orchestrator.execute(tree, query, options).await?;
//! ```

mod budget;
mod context;
mod orchestrator;
mod outcome;
mod stage;

pub use budget::BudgetStatus;
pub use context::{
    CandidateNode, PipelineContext, SearchAlgorithm, SearchConfig,
};
pub use orchestrator::RetrievalOrchestrator;
pub use outcome::StageOutcome;
pub use stage::RetrievalStage;

// Re-export FailurePolicy from index for convenience
pub use crate::index::pipeline::FailurePolicy;
