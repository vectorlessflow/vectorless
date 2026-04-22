// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index pipeline stages.

mod build;
mod concept;
mod enhance;
mod enrich;
mod navigation;
mod optimize;
mod parse;
mod reasoning;
mod split;
mod validate;

pub use build::BuildStage;
pub use concept::ConceptExtractionStage;
pub use enhance::EnhanceStage;
pub use enrich::EnrichStage;
pub use navigation::NavigationIndexStage;
pub use optimize::OptimizeStage;
pub use parse::ParseStage;
pub use reasoning::ReasoningIndexStage;
pub use split::SplitStage;
pub use validate::ValidateStage;

use super::pipeline::{FailurePolicy, IndexContext, StageResult};
use crate::error::Result;
pub use async_trait::async_trait;

/// Declares which context fields a stage reads/writes.
/// Used by the orchestrator to determine safe parallel execution.
#[derive(Debug, Clone, Default)]
pub struct AccessPattern {
    /// Whether this stage reads the tree.
    pub reads_tree: bool,
    /// Whether this stage mutates the tree (summaries, structure, etc.).
    pub writes_tree: bool,
    /// Whether this stage writes to `reasoning_index`.
    pub writes_reasoning_index: bool,
    /// Whether this stage writes to `navigation_index`.
    pub writes_navigation_index: bool,
    /// Whether this stage writes to `description`.
    pub writes_description: bool,
    /// Whether this stage writes to `concepts`.
    pub writes_concepts: bool,
}

/// Index pipeline stage.
///
/// Each stage represents a discrete step in the document indexing process.
/// Stages are executed in dependency order by the [`PipelineOrchestrator`].
///
/// # Stage Lifecycle
///
/// 1. Stage is registered with the orchestrator
/// 2. Dependencies are resolved and execution order is determined
/// 3. `execute()` is called with the shared context
/// 4. Results are stored in `ctx.stage_results`
///
/// # Example
///
/// ```rust,ignore
/// struct MyStage;
///
/// #[async_trait]
/// impl IndexStage for MyStage {
///     fn name(&self) -> &str { "my_stage" }
///
///     fn depends_on(&self) -> Vec<&'static str> {
///         vec!["parse", "build"]
///     }
///
///     async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
///         // Process the context...
///         Ok(StageResult::success("my_stage"))
///     }
/// }
/// ```
#[async_trait]
pub trait IndexStage: Send + Sync {
    /// Stage name (must be unique within pipeline).
    fn name(&self) -> &str;

    /// Execute the stage.
    ///
    /// This method receives a mutable reference to the shared context,
    /// allowing stages to read from and write to it.
    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult>;

    /// Whether this stage is optional (can be skipped on failure).
    ///
    /// Optional stages that fail will not stop the pipeline.
    /// Default: `false`
    fn is_optional(&self) -> bool {
        false
    }

    /// Names of stages this stage depends on.
    ///
    /// Dependencies are validated during pipeline construction.
    /// A stage will only execute after all its dependencies have completed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// fn depends_on(&self) -> Vec<&'static str> {
    ///     vec!["parse", "build"]
    /// }
    /// ```
    fn depends_on(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// Failure policy for this stage.
    ///
    /// Determines how the pipeline handles failures in this stage:
    /// - `Fail`: Stop the entire pipeline (default for required stages)
    /// - `Skip`: Skip this stage, continue pipeline
    /// - `Retry`: Retry with exponential backoff
    ///
    /// Default behavior:
    /// - If `is_optional()` returns true, defaults to `FailurePolicy::Skip`
    /// - Otherwise, defaults to `FailurePolicy::Fail`
    fn failure_policy(&self) -> FailurePolicy {
        if self.is_optional() {
            FailurePolicy::skip()
        } else {
            FailurePolicy::fail()
        }
    }

    /// Declare which context fields this stage accesses.
    /// Used by the orchestrator for safe parallel execution.
    fn access_pattern(&self) -> AccessPattern {
        AccessPattern::default()
    }
}
