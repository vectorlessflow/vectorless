// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pipeline-based retriever implementation.
//!
//! This module provides a `Retriever` implementation that uses the
//! new pipeline architecture (RetrievalOrchestrator) internally.

use async_trait::async_trait;
use std::sync::Arc;

use super::pipeline::RetrievalOrchestrator;
use super::retriever::{CostEstimate, Retriever, RetrieverError, RetrieverResult};
use super::stages::{AnalyzeStage, JudgeStage, PlanStage, SearchStage};
use super::strategy::LlmStrategy;
use super::types::{RetrieveOptions, RetrieveResponse};
use crate::domain::DocumentTree;
use crate::llm::LlmClient;

/// Pipeline-based retriever using the stage architecture.
///
/// This retriever uses the new pipeline architecture with:
/// - Analyze stage: Query complexity and keyword extraction
/// - Plan stage: Strategy and algorithm selection
/// - Search stage: Tree traversal
/// - Judge stage: Sufficiency checking
///
/// # Example
///
/// ```rust,ignore
/// let retriever = PipelineRetriever::new()
///     .with_llm_client(llm_client);
///
/// let response = retriever.retrieve(&tree, "query", &options).await?;
/// ```
pub struct PipelineRetriever {
    llm_client: Option<LlmClient>,
    max_backtracks: usize,
    max_iterations: usize,
}

impl Default for PipelineRetriever {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineRetriever {
    /// Create a new pipeline retriever.
    pub fn new() -> Self {
        Self {
            llm_client: None,
            max_backtracks: 5,
            max_iterations: 10,
        }
    }

    /// Add LLM client for enhanced retrieval.
    pub fn with_llm_client(mut self, client: LlmClient) -> Self {
        self.llm_client = Some(client);
        self
    }

    /// Set maximum backtracks for incremental retrieval.
    pub fn with_max_backtracks(mut self, n: usize) -> Self {
        self.max_backtracks = n;
        self
    }

    /// Set maximum total iterations.
    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    /// Build the orchestrator with all stages.
    fn build_orchestrator(&self) -> RetrievalOrchestrator {
        let mut orchestrator = RetrievalOrchestrator::new()
            .with_max_backtracks(self.max_backtracks)
            .with_max_iterations(self.max_iterations);

        // Add analyze stage
        orchestrator = orchestrator.stage(AnalyzeStage::new());

        // Add plan stage
        let mut plan_stage = PlanStage::new();
        if let Some(ref client) = self.llm_client {
            plan_stage = plan_stage.with_llm_client(client.clone());
        }
        orchestrator = orchestrator.stage(plan_stage);

        // Add search stage
        let mut search_stage = SearchStage::new();
        if let Some(ref client) = self.llm_client {
            search_stage = search_stage.with_llm_strategy(LlmStrategy::new(client.clone()));
        }
        orchestrator = orchestrator.stage(search_stage);

        // Add judge stage
        let mut judge_stage = JudgeStage::new();
        if let Some(ref client) = self.llm_client {
            judge_stage = judge_stage.with_llm_judge(client.clone());
        }
        orchestrator = orchestrator.stage(judge_stage);

        orchestrator
    }

    /// Convert pipeline options to retriever options format.
    fn options_to_retrieve_options(&self, options: &RetrieveOptions) -> RetrieveOptions {
        options.clone()
    }
}

#[async_trait]
impl Retriever for PipelineRetriever {
    async fn retrieve(
        &self,
        tree: &DocumentTree,
        query: &str,
        options: &RetrieveOptions,
    ) -> RetrieverResult<RetrieveResponse> {
        let mut orchestrator = self.build_orchestrator();
        let tree_arc = Arc::new(tree.clone());

        // Execute the pipeline
        let response = orchestrator
            .execute(tree_arc, query, self.options_to_retrieve_options(options))
            .await
            .map_err(|e| RetrieverError::Internal(e.to_string()))?;

        Ok(response)
    }

    fn name(&self) -> &str {
        "pipeline"
    }

    fn supports_options(&self, _options: &RetrieveOptions) -> bool {
        true // Pipeline retriever supports all options
    }

    fn estimate_cost(&self, tree: &DocumentTree, options: &RetrieveOptions) -> CostEstimate {
        // Estimate based on tree size and options
        let node_count = tree.node_count();
        let base_llm_calls = if options.sufficiency_check { 2 } else { 1 };

        CostEstimate {
            llm_calls: base_llm_calls + (node_count / 10), // Rough estimate
            tokens: node_count * 50, // Conservative estimate
        }
    }
}

impl Clone for PipelineRetriever {
    fn clone(&self) -> Self {
        Self {
            llm_client: self.llm_client.clone(),
            max_backtracks: self.max_backtracks,
            max_iterations: self.max_iterations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_retriever_creation() {
        let retriever = PipelineRetriever::new();
        assert_eq!(retriever.name(), "pipeline");
        assert!(retriever.llm_client.is_none());
    }

    #[test]
    fn test_pipeline_retriever_clone() {
        let retriever = PipelineRetriever::new().with_max_backtracks(3);
        let cloned = retriever.clone();
        assert_eq!(cloned.name(), "pipeline");
        assert_eq!(cloned.max_backtracks, 3);
    }
}
