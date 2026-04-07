// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Enhance stage - Generate summaries using LLM.

use super::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::document::{DocumentTree, NodeId, TreeNode};
use crate::error::Result;
use crate::utils::fingerprint::Fingerprint;
use crate::llm::LlmClient;
use crate::memo::{MemoKey, MemoStore, MemoValue};

use super::{IndexStage, StageResult};
use crate::index::pipeline::{FailurePolicy, IndexContext, StageRetryConfig};
use crate::index::summary::{LlmSummaryGenerator, SummaryGenerator, SummaryStrategy};

/// Enhance stage - generates summaries using LLM.
pub struct EnhanceStage {
    /// LLM client for summary generation.
    llm_client: Option<Arc<LlmClient>>,
    /// Memo store for caching LLM results.
    memo_store: Option<Arc<MemoStore>>,
}

impl EnhanceStage {
    /// Create a new enhance stage.
    pub fn new() -> Self {
        Self {
            llm_client: None,
            memo_store: None,
        }
    }

    /// Create with LLM client.
    pub fn with_llm_client(client: LlmClient) -> Self {
        Self {
            llm_client: Some(Arc::new(client)),
            memo_store: None,
        }
    }

    /// Create with LLM client and memo store.
    pub fn with_llm_and_memo(client: LlmClient, memo_store: MemoStore) -> Self {
        Self {
            llm_client: Some(Arc::new(client)),
            memo_store: Some(Arc::new(memo_store)),
        }
    }

    /// Set memo store for caching.
    pub fn with_memo_store(mut self, store: MemoStore) -> Self {
        self.memo_store = Some(Arc::new(store));
        self
    }

    /// Check if summary generation is needed based on strategy.
    fn needs_summaries(&self, ctx: &IndexContext) -> bool {
        match &ctx.options.summary_strategy {
            SummaryStrategy::None => false,
            SummaryStrategy::Lazy { .. } => false, // Generated on-demand at query time
            SummaryStrategy::Full { .. } | SummaryStrategy::Selective { .. } => true,
        }
    }
}

impl Default for EnhanceStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexStage for EnhanceStage {
    fn name(&self) -> &'static str {
        "enhance"
    }

    fn is_optional(&self) -> bool {
        true
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["build"]
    }

    fn failure_policy(&self) -> FailurePolicy {
        // LLM operations benefit from retry with backoff
        FailurePolicy::retry_with(
            StageRetryConfig::new()
                .with_max_attempts(2)
                .with_initial_delay(Duration::from_millis(500)),
        )
    }

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        let start = Instant::now();

        // Check if we need summaries
        if !self.needs_summaries(ctx) {
            info!(
                "Summary generation skipped (strategy: {:?})",
                ctx.options.summary_strategy
            );
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

        // Create summary generator with optional memo store
        let mut generator = LlmSummaryGenerator::new((*llm_client).as_ref().clone())
            .with_max_tokens(ctx.options.indexer.max_summary_tokens);

        // Attach memo store to generator if available
        if let Some(store) = &self.memo_store {
            generator = generator.with_memo_store((**store).clone());
        }

        // Get all nodes to process
        let node_ids: Vec<NodeId> = tree.traverse();
        let total_nodes = node_ids.len();

        info!("Processing {} nodes for summary generation", total_nodes);

        // Process nodes
        let mut generated = 0;
        let mut failed = 0;
        let strategy = ctx.options.summary_strategy.clone();

        for node_id in node_ids {
            // Get node data (need to clone to avoid borrow issues)
            let node = match tree.get(node_id) {
                Some(n) => n.clone(),
                None => continue,
            };

            // Skip if no content
            if node.content.is_empty() {
                continue;
            }

            // Get token count and check if we should generate
            let token_count = node.token_count.unwrap_or(0);
            if !strategy.should_generate(tree, node_id, token_count) {
                continue;
            }

            // Check memo store first (additional check beyond generator)
            let cached_summary = if let Some(store) = self.memo_store.as_deref() {
                let content_fp =
                    Fingerprint::from_str(&format!("{}|{}", node.title, node.content));
                let memo_key = MemoKey::summary(&content_fp);

                store
                    .get(&memo_key)
                    .and_then(|cached| cached.as_summary().map(|s| s.to_string()))
            } else {
                None
            };

            if let Some(summary) = cached_summary {
                if !summary.is_empty() {
                    tree.set_summary(node_id, &summary);
                    debug!(
                        "Using cached summary for node: {} ({} chars)",
                        node.title,
                        summary.len()
                    );
                    ctx.metrics.increment_summaries();
                    generated += 1;
                    continue;
                }
            }

            // Generate summary (generator also has memoization built-in)
            match generator.generate(&node.title, &node.content).await {
                Ok(summary) => {
                    if summary.is_empty() {
                        warn!("Empty summary returned for node '{}'", node.title);
                        failed += 1;
                    } else {
                        tree.set_summary(node_id, &summary);
                        debug!(
                            "Generated summary for node: {} ({} chars)",
                            node.title,
                            summary.len()
                        );
                        ctx.metrics.increment_summaries();
                        generated += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to generate summary for {}: {}", node.title, e);
                    failed += 1;
                }
            }

            // Increment LLM calls metric
            ctx.metrics.increment_llm_calls();
        }

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_enhance(duration);

        info!(
            "Generated {} summaries ({} failed) in {}ms",
            generated, failed, duration
        );

        let mut stage_result = StageResult::success("enhance");
        stage_result.duration_ms = duration;
        stage_result.metadata.insert(
            "summaries_generated".to_string(),
            serde_json::json!(generated),
        );
        stage_result
            .metadata
            .insert("summaries_failed".to_string(), serde_json::json!(failed));

        Ok(stage_result)
    }
}
