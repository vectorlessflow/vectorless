// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Enhance stage - Generate summaries using LLM.

use super::async_trait;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

use crate::domain::{NodeId, Result, DocumentTree};
use crate::llm::LlmClient;

use super::{IndexStage, StageResult};
use crate::index::pipeline::IndexContext;
use crate::index::summary::{LlmSummaryGenerator, SummaryGenerator, SummaryStrategy};

/// Enhance stage - generates summaries using LLM.
pub struct EnhanceStage {
    /// LLM client for summary generation.
    llm_client: Option<Arc<LlmClient>>,
}

impl EnhanceStage {
    /// Create a new enhance stage.
    pub fn new() -> Self {
        Self { llm_client: None }
    }

    /// Create with LLM client.
    pub fn with_llm_client(client: LlmClient) -> Self {
        Self {
            llm_client: Some(Arc::new(client)),
        }
    }

    /// Check if summary generation is needed.
    fn needs_summaries(&self, ctx: &IndexContext) -> bool {
        match &ctx.options.summary_strategy {
            SummaryStrategy::None => false,
            SummaryStrategy::Lazy { .. } => false,
            _ => true,
        }
    }

    /// Generate summary for a single node.
    async fn generate_node_summary(
        tree: &mut DocumentTree,
        node_id: NodeId,
        generator: &LlmSummaryGenerator,
        strategy: &SummaryStrategy,
        metrics: &mut crate::index::IndexMetrics,
    ) -> Result<()> {
        let node = match tree.get(node_id) {
            Some(n) => n.clone(),
            None => return Ok(()),
        };

        // Skip if no content
        if node.content.is_empty() {
            return Ok(());
        }

        // Get token count
        let token_count = node.token_count.unwrap_or(0);
        let should_gen = strategy.should_generate(tree, node_id, token_count);

        // Check if we should generate
        if !should_gen {
            return Ok(());
        }

        // Generate summary
        match generator.generate(&node.title, &node.content).await {
            Ok(summary) => {
                if !summary.is_empty() {
                    tree.set_summary(node_id, &summary);
                    info!("Generated summary for node: {} ({} chars)", node.title, summary.len());
                    metrics.increment_summaries();
                } else {
                    warn!("Empty summary returned for node '{}'", node.title);
                }
            }
            Err(e) => {
                warn!("Failed to generate summary for {}: {}", node.title, e);
            }
        }

        Ok(())
    }
}

impl Default for EnhanceStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexStage for EnhanceStage {
    fn name(&self) -> &str {
        "enhance"
    }

    fn is_optional(&self) -> bool {
        true
    }

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        let start = Instant::now();

        // Check if we need summaries
        if !self.needs_summaries(ctx) {
            info!("Summary generation skipped (strategy: {:?})", ctx.options.summary_strategy);
            return Ok(StageResult::success("enhance"));
        }

        // Get LLM client
        let llm_client = match &self.llm_client {
            Some(client) => client,
            None => {
                warn!("No LLM client configured, skipping summary generation");
                return Ok(StageResult::success("enhance"));
            }
        };

        // Get tree
        let tree = match ctx.tree.as_mut() {
            Some(t) => t,
            None => {
                warn!("No tree built, skipping enhance stage");
                return Ok(StageResult::success("enhance"));
            }
        };

        info!("Using summary strategy: {:?}", ctx.options.summary_strategy);

        // Create summary generator
        let generator = LlmSummaryGenerator::new((*llm_client).as_ref().clone())
            .with_max_tokens(ctx.options.indexer.max_summary_tokens);

        // Get all nodes to process
        let node_ids: Vec<NodeId> = tree.traverse();
        let total_nodes = node_ids.len();

        info!("Processing {} nodes for summary generation", total_nodes);

        // Process nodes (with concurrency control)
        let mut generated = 0;
        let mut failed = 0;
        let strategy = ctx.options.summary_strategy.clone();

        for node_id in node_ids {
            match Self::generate_node_summary(tree, node_id, &generator, &strategy, &mut ctx.metrics).await {
                Ok(()) => {
                    generated += 1;
                }
                Err(e) => {
                    failed += 1;
                    warn!("Failed to generate summary: {}", e);
                }
            }

            // Increment LLM calls metric
            ctx.metrics.increment_llm_calls();
        }

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_enhance(duration);

        info!(
            "Generated {} summaries ({} failed) in {}ms",
            generated,
            failed,
            duration
        );

        let mut stage_result = StageResult::success("enhance");
        stage_result.duration_ms = duration;
        stage_result.metadata.insert(
            "summaries_generated".to_string(),
            serde_json::json!(generated),
        );
        stage_result.metadata.insert(
            "summaries_failed".to_string(),
            serde_json::json!(failed),
        );

        Ok(stage_result)
    }
}
