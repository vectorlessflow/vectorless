// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pipeline executor for running index stages.
//!
//! The executor uses [`PipelineOrchestrator`] internally for flexible
//! stage management with priority-based ordering and dependency resolution.

use tracing::info;

use crate::error::Result;
use crate::llm::LlmClient;

use super::super::PipelineOptions;
use super::super::stages::{
    BuildStage, EnhanceStage, EnrichStage, IndexStage, OptimizeStage, ParseStage,
    ReasoningIndexStage, SplitStage, ValidateStage,
};
use super::context::{IndexInput, PipelineResult};
use super::orchestrator::PipelineOrchestrator;

/// Pipeline executor for document indexing.
///
/// Uses [`PipelineOrchestrator`] internally for stage management.
/// Supports both preset configurations and custom stage pipelines.
///
/// # Example
///
/// ```rust,ignore
/// // Default pipeline
/// let executor = PipelineExecutor::new();
/// let result = executor.execute(input, options).await?;
///
/// // With LLM enhancement
/// let executor = PipelineExecutor::with_llm(client);
///
/// // Custom pipeline using orchestrator
/// let orchestrator = PipelineOrchestrator::new()
///     .stage(ParseStage::new())
///     .stage_with_priority(MyCustomStage::new(), 50)
///     .stage(BuildStage::new());
/// let executor = PipelineExecutor::from_orchestrator(orchestrator);
/// ```
pub struct PipelineExecutor {
    orchestrator: PipelineOrchestrator,
}

impl PipelineExecutor {
    /// Create a new pipeline executor with default stages.
    ///
    /// Default stages (in order):
    /// 1. `parse` - Parse document into raw nodes
    /// 2. `build` - Build tree structure
    /// 3. `validate` - Verify tree integrity (optional)
    /// 4. `split` - Split oversized leaf nodes (optional)
    /// 5. `enrich` - Add metadata and cross-references
    /// 6. `reasoning_index` - Build pre-computed reasoning index
    /// 7. `optimize` - Optimize tree structure
    pub fn new() -> Self {
        let orchestrator = PipelineOrchestrator::new()
            .stage_with_priority(ParseStage::new(), 10)
            .stage_with_priority(BuildStage::new(), 20)
            .stage_with_priority(ValidateStage::new(), 22)
            .stage_with_priority(SplitStage::new(), 25)
            .stage_with_priority(EnrichStage::new(), 40)
            .stage_with_priority(ReasoningIndexStage::new(), 45)
            .stage_with_priority(OptimizeStage::new(), 60);

        Self { orchestrator }
    }

    /// Create a pipeline with LLM enhancement.
    ///
    /// Stages (in order):
    /// 1. `parse` - Parse document
    /// 2. `build` - Build tree
    /// 3. `validate` - Verify tree integrity (optional)
    /// 4. `split` - Split oversized leaf nodes (optional)
    /// 5. `enhance` - LLM-based enhancement (summaries)
    /// 6. `enrich` - Add metadata
    /// 7. `reasoning_index` - Build pre-computed reasoning index
    /// 8. `optimize` - Optimize tree
    pub fn with_llm(client: LlmClient) -> Self {
        let orchestrator = PipelineOrchestrator::new()
            .stage_with_priority(ParseStage::new(), 10)
            .stage_with_priority(BuildStage::new(), 20)
            .stage_with_priority(ValidateStage::new(), 22)
            .stage_with_priority(SplitStage::new(), 25)
            .stage_with_priority(EnhanceStage::with_llm_client(client), 30)
            .stage_with_priority(EnrichStage::new(), 40)
            .stage_with_priority(ReasoningIndexStage::new(), 45)
            .stage_with_priority(OptimizeStage::new(), 60);

        Self { orchestrator }
    }

    /// Create from a custom orchestrator.
    ///
    /// Use this for full control over stage ordering and dependencies.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let orchestrator = PipelineOrchestrator::new()
    ///     .stage_with_priority(ParseStage::new(), 10)
    ///     .stage_with_priority(MyAnalysisStage::new(), 25)
    ///     .stage_with_priority(BuildStage::new(), 20)
    ///     .stage_with_deps(MyValidationStage::new(), 50, &["build"]);
    ///
    /// let executor = PipelineExecutor::from_orchestrator(orchestrator);
    /// ```
    pub fn from_orchestrator(orchestrator: PipelineOrchestrator) -> Self {
        Self { orchestrator }
    }

    /// Add a stage with default priority.
    ///
    /// The stage will be added after existing stages with the same priority.
    pub fn add_stage(mut self, stage: impl IndexStage + 'static) -> Self {
        self.orchestrator = self.orchestrator.stage(stage);
        self
    }

    /// Add a stage with custom priority.
    ///
    /// Lower priority = earlier execution.
    pub fn add_stage_with_priority(
        mut self,
        stage: impl IndexStage + 'static,
        priority: i32,
    ) -> Self {
        self.orchestrator = self.orchestrator.stage_with_priority(stage, priority);
        self
    }

    /// Add a stage with priority and dependencies.
    ///
    /// The stage will run after all specified dependencies.
    pub fn add_stage_with_deps(
        mut self,
        stage: impl IndexStage + 'static,
        priority: i32,
        depends_on: &[&str],
    ) -> Self {
        self.orchestrator = self
            .orchestrator
            .stage_with_deps(stage, priority, depends_on);
        self
    }

    /// Get the list of stage names in execution order.
    pub fn stage_names(&self) -> Result<Vec<&str>> {
        self.orchestrator.stage_names()
    }

    /// Get the number of stages.
    pub fn stage_count(&self) -> usize {
        self.orchestrator.stage_count()
    }

    /// Execute the pipeline.
    ///
    /// Stages are executed in dependency-resolved order.
    pub async fn execute(
        &mut self,
        input: IndexInput,
        options: PipelineOptions,
    ) -> Result<PipelineResult> {
        info!(
            "Starting index pipeline with {} stages",
            self.orchestrator.stage_count()
        );
        self.orchestrator.execute(input, options).await
    }
}

impl Default for PipelineExecutor {
    fn default() -> Self {
        Self::new()
    }
}
