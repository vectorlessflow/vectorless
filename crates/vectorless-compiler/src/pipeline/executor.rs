// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pipeline executor for running compile passes.
//!
//! The executor uses [`PipelineOrchestrator`] internally for flexible
//! stage management with priority-based ordering and dependency resolution.

use tracing::info;

use vectorless_error::Result;
use vectorless_llm::LlmClient;

use super::super::PipelineOptions;
use super::super::parse::{Parser, ParserRegistry};
use super::super::passes::{
    BuildPass, ChainPass, CompilePass, ConceptPass, EnhancePass, EnrichPass, NavigationPass,
    OptimizePass, OverlapPass, ParsePass, ReasoningPass, RoutePass, ScorePass, SplitPass,
    ValidatePass, VerifyPass,
};
use super::context::{CompileResult, CompilerInput};
use super::orchestrator::PipelineOrchestrator;

/// Pipeline executor for document compilation.
///
/// Uses [`PipelineOrchestrator`] internally for pass management.
/// Supports both preset configurations and custom pass pipelines.
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
///     .stage(ParsePass::new())
///     .stage_with_priority(MyCustomStage::new(), 50)
///     .stage(BuildPass::new());
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
    /// 7. `concept_extraction` - Extract key concepts (optional)
    /// 8. `navigation_index` - Build Agent navigation index
    /// 9. `verify` - Validate ingest output reliability
    /// 10. `optimize` - Optimize tree structure
    pub fn new() -> Self {
        let orchestrator = PipelineOrchestrator::new()
            .stage_with_priority(ParsePass::new(), 10)
            .stage_with_priority(BuildPass::new(), 20)
            .stage_with_priority(ValidatePass::new(), 22)
            .stage_with_priority(SplitPass::new(), 25)
            .stage_with_priority(EnrichPass::new(), 40)
            .stage_with_priority(ReasoningPass::new(), 45)
            .stage_with_priority(ConceptPass::new(), 47)
            .stage_with_priority(NavigationPass::new(), 50)
            .stage_with_priority(RoutePass::new(), 52)
            .stage_with_priority(ChainPass::new(), 54)
            .stage_with_priority(OverlapPass::new(), 56)
            .stage_with_priority(ScorePass::new(), 58)
            .stage_with_priority(VerifyPass, 55)
            .stage_with_priority(OptimizePass::new(), 60);

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
    /// 8. `concept_extraction` - Extract key concepts via LLM (optional)
    /// 9. `navigation_index` - Build Agent navigation index
    /// 10. `verify` - Validate ingest output reliability
    /// 11. `optimize` - Optimize tree
    pub fn with_llm(client: LlmClient) -> Self {
        tracing::info!(
            "PipelineExecutor::with_llm — cloning client to ParsePass + EnhancePass + context"
        );
        let orchestrator = PipelineOrchestrator::new()
            .with_llm_client(client.clone())
            .stage_with_priority(ParsePass::with_llm_client(client.clone()), 10)
            .stage_with_priority(BuildPass::new(), 20)
            .stage_with_priority(ValidatePass::new(), 22)
            .stage_with_priority(SplitPass::new(), 25)
            .stage_with_priority(EnhancePass::with_llm_client(client.clone()), 30)
            .stage_with_priority(EnrichPass::new(), 40)
            .stage_with_priority(ReasoningPass::new(), 45)
            .stage_with_priority(ConceptPass::with_llm_client(client), 47)
            .stage_with_priority(NavigationPass::new(), 50)
            .stage_with_priority(RoutePass::new(), 52)
            .stage_with_priority(ChainPass::new(), 54)
            .stage_with_priority(OverlapPass::new(), 56)
            .stage_with_priority(ScorePass::new(), 58)
            .stage_with_priority(VerifyPass, 55)
            .stage_with_priority(OptimizePass::new(), 60);

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
    ///     .stage_with_priority(ParsePass::new(), 10)
    ///     .stage_with_priority(MyAnalysisStage::new(), 25)
    ///     .stage_with_priority(BuildPass::new(), 20)
    ///     .stage_with_deps(MyValidationStage::new(), 50, &["build"]);
    ///
    /// let executor = PipelineExecutor::from_orchestrator(orchestrator);
    /// ```
    pub fn from_orchestrator(orchestrator: PipelineOrchestrator) -> Self {
        Self { orchestrator }
    }

    /// Create with a custom parser registry.
    ///
    /// Use this to register custom format parsers alongside the built-in
    /// Markdown and PDF parsers.
    pub fn with_registry(registry: ParserRegistry) -> Self {
        let orchestrator = PipelineOrchestrator::new()
            .stage_with_priority(ParsePass::with_registry(registry), 10)
            .stage_with_priority(BuildPass::new(), 20)
            .stage_with_priority(ValidatePass::new(), 22)
            .stage_with_priority(SplitPass::new(), 25)
            .stage_with_priority(EnrichPass::new(), 40)
            .stage_with_priority(ReasoningPass::new(), 45)
            .stage_with_priority(ConceptPass::new(), 47)
            .stage_with_priority(NavigationPass::new(), 50)
            .stage_with_priority(RoutePass::new(), 52)
            .stage_with_priority(ChainPass::new(), 54)
            .stage_with_priority(OverlapPass::new(), 56)
            .stage_with_priority(ScorePass::new(), 58)
            .stage_with_priority(VerifyPass, 55)
            .stage_with_priority(OptimizePass::new(), 60);
        Self { orchestrator }
    }

    /// Add a single custom parser.
    ///
    /// Creates a default registry with built-in parsers plus the provided one.
    pub fn with_parser(parser: impl Parser + 'static) -> Self {
        let registry = ParserRegistry::default_parsers(None).with(parser);
        Self::with_registry(registry)
    }

    /// Add a stage with default priority.
    ///
    /// The stage will be added after existing stages with the same priority.
    pub fn add_stage(mut self, stage: impl CompilePass + 'static) -> Self {
        self.orchestrator = self.orchestrator.stage(stage);
        self
    }

    /// Add a stage with custom priority.
    ///
    /// Lower priority = earlier execution.
    pub fn add_stage_with_priority(
        mut self,
        stage: impl CompilePass + 'static,
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
        stage: impl CompilePass + 'static,
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
        input: CompilerInput,
        options: PipelineOptions,
    ) -> Result<CompileResult> {
        info!(
            "Starting compile pipeline with {} passes",
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
