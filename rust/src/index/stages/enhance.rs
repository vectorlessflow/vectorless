// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Enhance stage - Generate summaries using LLM.

use super::async_trait;
use futures::StreamExt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::document::NodeId;
use crate::error::Result;
use crate::index::incremental;
use crate::utils::fingerprint::Fingerprint;
use crate::llm::LlmClient;
use crate::memo::{MemoKey, MemoStore};

use super::{IndexStage, StageResult};
use crate::index::pipeline::{FailurePolicy, IndexContext, StageRetryConfig};
use crate::index::summary::{LlmSummaryGenerator, SummaryGenerator, SummaryStrategy};

/// A node that needs LLM summary generation.
struct PendingNode {
    node_id: NodeId,
    title: String,
    content: String,
    is_leaf: bool,
}

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

        // Create summary generator (shared via Arc for concurrent use)
        let generator = Arc::new(
            LlmSummaryGenerator::new((*llm_client).as_ref().clone())
                .with_max_tokens(ctx.options.indexer.max_summary_tokens)
                .with_memo_store(
                    self.memo_store
                        .as_ref()
                        .map(|s| (**s).clone())
                        .unwrap_or_default(),
                ),
        );

        // Get all nodes to process
        let node_ids: Vec<NodeId> = tree.traverse();
        let total_nodes = node_ids.len();

        // === Incremental: reuse summaries from existing tree for unchanged nodes ===
        if let Some(ref old_tree) = ctx.existing_tree {
            let reusable = incremental::compute_reusable_summaries(old_tree, tree);
            let applied = incremental::apply_reusable_summaries(tree, &reusable);
            for _ in 0..applied {
                ctx.metrics.increment_summaries();
            }
            info!(
                "Incremental: {} of {} nodes unchanged, reusing summaries",
                applied, total_nodes,
            );
        }

        info!("Processing {} nodes for summary generation", total_nodes);

        // === Phase 1: Collect pending nodes (cache hits applied immediately) ===
        let strategy = ctx.options.summary_strategy.clone();
        let mut pending_llm: Vec<PendingNode> = Vec::new();
        let mut generated = 0;
        let mut skipped_no_content = 0;
        let mut skipped_tokens = 0;
        let mut shortcut_used = 0;
        let shortcut_threshold = strategy.shortcut_threshold();

        for node_id in node_ids {
            let node = match tree.get(node_id) {
                Some(n) => n.clone(),
                None => continue,
            };

            // Skip if no content
            if node.content.is_empty() {
                skipped_no_content += 1;
                continue;
            }

            // Skip if summary already set (incremental: reused from old tree)
            if !node.summary.is_empty() {
                continue;
            }

            // Check if strategy says we should generate
            let token_count = node.token_count.unwrap_or(0);
            if !strategy.should_generate(tree, node_id, token_count) {
                skipped_tokens += 1;
                continue;
            }

            // Check memo store (fast path — apply immediately)
            if let Some(store) = self.memo_store.as_deref() {
                let content_fp =
                    Fingerprint::from_str(&format!("{}|{}", node.title, node.content));
                let memo_key = MemoKey::summary(&content_fp);
                if let Some(cached) = store.get(&memo_key).and_then(|c| c.as_summary().map(|s| s.to_string())) {
                    if !cached.is_empty() {
                        tree.set_summary(node_id, &cached);
                        debug!("Using cached summary for node: {} ({} chars)", node.title, cached.len());
                        ctx.metrics.increment_summaries();
                        generated += 1;
                        continue;
                    }
                }
            }

            // Shortcut: use original content as summary for short nodes (Borrow A)
            let token_count = node.token_count.unwrap_or_else(|| crate::utils::estimate_tokens(&node.content));
            if shortcut_threshold > 0 && token_count > 0 && token_count <= shortcut_threshold {
                tree.set_summary(node_id, &node.content);
                debug!(
                    "Shortcut: using original content as summary for '{}' ({} tokens)",
                    node.title, token_count
                );
                ctx.metrics.increment_summaries();
                generated += 1;
                shortcut_used += 1;
                continue;
            }

            // Needs LLM call
            let is_leaf = tree.is_leaf(node_id);
            pending_llm.push(PendingNode {
                node_id,
                title: node.title,
                content: node.content,
                is_leaf,
            });
        }

        // === Phase 2: Concurrent LLM calls with buffer_unordered ===
        let mut failed = 0;
        let concurrency = ctx.options.concurrency.max_concurrent_requests;

        if !pending_llm.is_empty() {
            info!(
                "Generating summaries for {} nodes (concurrency: {})",
                pending_llm.len(), concurrency
            );

            // Collect results: (NodeId, Result<String>)
            let results: Vec<(NodeId, std::result::Result<String, String>)> =
                futures::stream::iter(pending_llm)
                    .map(|pending| {
                        let generator = Arc::clone(&generator);
                        async move {
                            let result = generator.generate_for_node(
                                &pending.title,
                                &pending.content,
                                pending.is_leaf,
                            ).await;
                            (pending.node_id, result.map_err(|e| e.to_string()))
                        }
                    })
                    .buffer_unordered(concurrency)
                    .collect()
                    .await;

            // Write results back to tree
            for (node_id, result) in results {
                ctx.metrics.increment_llm_calls();
                match result {
                    Ok(summary) => {
                        if summary.is_empty() {
                            failed += 1;
                        } else {
                            tree.set_summary(node_id, &summary);
                            generated += 1;
                            ctx.metrics.increment_summaries();
                        }
                    }
                    Err(e) => {
                        warn!("Failed to generate summary: {}", e);
                        failed += 1;
                    }
                }
            }
        }

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_enhance(duration);

        info!(
            "Generated {} summaries ({} shortcut, {} failed, {} skipped no content, {} skipped tokens) in {}ms",
            generated, shortcut_used, failed, skipped_no_content, skipped_tokens, duration
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
