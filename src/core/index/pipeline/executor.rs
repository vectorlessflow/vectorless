// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pipeline executor for running index stages.

use std::time::Instant;
use tracing::{info, warn, error};

use crate::core::Result;
use crate::llm::LlmClient;

use super::context::{IndexContext, IndexInput, IndexResult};
use super::super::stages::{
    BuildStage, EnhanceStage, EnrichStage, IndexStage, OptimizeStage,
    ParseStage, PersistStage,
};
use super::super::PipelineOptions;

/// Pipeline executor for document indexing.
pub struct PipelineExecutor {
    stages: Vec<Box<dyn IndexStage>>,
}

impl PipelineExecutor {
    /// Create a new pipeline executor with default stages.
    pub fn new() -> Self {
        Self {
            stages: vec![
                Box::new(ParseStage::new()),
                Box::new(BuildStage::new()),
                Box::new(EnrichStage::new()),
                Box::new(OptimizeStage::new()),
            ],
        }
    }

    /// Create a pipeline with LLM enhancement.
    pub fn with_llm(client: LlmClient) -> Self {
        Self {
            stages: vec![
                Box::new(ParseStage::new()),
                Box::new(BuildStage::new()),
                Box::new(EnhanceStage::with_llm_client(client)),
                Box::new(EnrichStage::new()),
                Box::new(OptimizeStage::new()),
            ],
        }
    }

    /// Add a stage to the pipeline.
    pub fn add_stage(mut self, stage: Box<dyn IndexStage>) -> Self {
        self.stages.push(stage);
        self
    }

    /// Add persistence stage with workspace.
    pub fn with_persistence(mut self, workspace: crate::storage::Workspace) -> Self {
        self.stages.push(Box::new(PersistStage::with_workspace(workspace)));
        self
    }

    /// Execute the pipeline.
    pub async fn execute(&mut self, input: IndexInput, options: PipelineOptions) -> Result<IndexResult> {
        let total_start = Instant::now();
        info!("Starting index pipeline");

        // Create context
        let mut ctx = IndexContext::new(input, options);

        // Execute each stage
        for stage in &mut self.stages {
            let stage_name = stage.name().to_string();
            let is_optional = stage.is_optional();
            info!("Executing stage: {}", stage_name);

            match stage.execute(&mut ctx).await {
                Ok(result) => {
                    ctx.stage_results.insert(stage_name.clone(), result);
                }
                Err(e) => {
                    if is_optional {
                        warn!("Optional stage {} failed: {}", stage_name, e);
                    } else {
                        error!("Required stage {} failed: {}", stage_name, e);
                        return Err(e);
                    }
                }
            }
        }

        let total_duration = total_start.elapsed().as_millis() as u64;
        info!(
            "Index pipeline completed in {}ms for document {}",
            total_duration, ctx.name
        );

        // Finalize result
        Ok(ctx.finalize())
    }
}

impl Default for PipelineExecutor {
    fn default() -> Self {
        Self::new()
    }
}
