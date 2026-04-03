// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval stage trait definition.
//!
//! Defines the [`RetrievalStage`] trait for pipeline stages, similar to
//! [`IndexStage`](crate::index::stages::IndexStage) but with additional
//! capabilities for backtracking and incremental retrieval.

use async_trait::async_trait;

use crate::domain::Result;
use crate::index::pipeline::FailurePolicy;

use super::context::PipelineContext;
use super::outcome::StageOutcome;

/// Retrieval pipeline stage.
///
/// Each stage represents a discrete step in the retrieval process.
/// Unlike indexing stages, retrieval stages can trigger backtracking
/// or request additional data collection.
///
/// # Stage Lifecycle
///
/// 1. Stage is registered with the orchestrator
/// 2. Dependencies are resolved and execution order is determined
/// 3. `execute()` is called with the shared context
/// 4. Returns `StageOutcome` to control pipeline flow
///
/// # Example
///
/// ```rust,ignore
/// struct MyStage;
///
/// #[async_trait]
/// impl RetrievalStage for MyStage {
///     fn name(&self) -> &str { "my_stage" }
///
///     fn depends_on(&self) -> Vec<&'static str> {
///         vec!["analyze"]
///     }
///
///     async fn execute(&self, ctx: &mut PipelineContext) -> Result<StageOutcome> {
///         // Process the context...
///         Ok(StageOutcome::cont())
///     }
/// }
/// ```
#[async_trait]
pub trait RetrievalStage: Send + Sync {
    /// Stage name (must be unique within pipeline).
    fn name(&self) -> &str;

    /// Execute the stage.
    ///
    /// This method receives a mutable reference to the shared context,
    /// allowing stages to read from and write to it.
    ///
    /// Returns a `StageOutcome` to control pipeline flow:
    /// - `Continue`: Proceed to next stage
    /// - `Complete`: Retrieval is done, return results
    /// - `NeedMoreData`: Go back to search for more data
    /// - `Backtrack`: Return to a specific stage
    /// - `Skip`: Skip remaining stages
    async fn execute(&self, ctx: &mut PipelineContext) -> Result<StageOutcome>;

    /// Names of stages this stage depends on.
    ///
    /// Dependencies are validated during pipeline construction.
    /// A stage will only execute after all its dependencies have completed.
    fn depends_on(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// Whether this stage is optional (can be skipped on failure).
    ///
    /// Optional stages that fail will not stop the pipeline.
    /// Default: `false`
    fn is_optional(&self) -> bool {
        false
    }

    /// Failure policy for this stage.
    ///
    /// Determines how the pipeline handles failures in this stage:
    /// - `Fail`: Stop the entire pipeline (default for required stages)
    /// - `Skip`: Skip this stage, continue pipeline
    /// - `Retry`: Retry with exponential backoff
    fn failure_policy(&self) -> FailurePolicy {
        if self.is_optional() {
            FailurePolicy::skip()
        } else {
            FailurePolicy::fail()
        }
    }

    /// Whether this stage can trigger backtracking.
    ///
    /// Stages like Judge that evaluate sufficiency may need to
    /// trigger additional search iterations.
    fn can_backtrack(&self) -> bool {
        false
    }

    /// Priority for ordering (lower = earlier).
    ///
    /// Used when stages have no dependency relationship.
    /// Default: 100
    fn priority(&self) -> i32 {
        100
    }
}
