// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Enhance stage - Generate summaries using LLM.

use crate::passes::async_trait;
use futures::StreamExt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::incremental;
use vectorless_document::NodeId;
use vectorless_error::Result;
use vectorless_llm::LlmClient;
use vectorless_llm::memo::{MemoKey, MemoStore};
use vectorless_utils::fingerprint::Fingerprint;

use crate::passes::{CompilePass, PassResult};
use crate::pipeline::{CompileContext, FailurePolicy, StageRetryConfig};
use crate::summary::{LlmSummaryGenerator, SummaryGenerator, SummaryStrategy};

/// A node that needs LLM summary generation.
struct PendingNode {
    node_id: NodeId,
    title: String,
    content: String,
    is_leaf: bool,
}

/// Enhance stage - generates summaries using LLM.
pub struct EnhancePass {
    /// LLM client for summary generation.
    llm_client: Option<Arc<LlmClient>>,
    /// Memo store for caching LLM results.
    memo_store: Option<Arc<MemoStore>>,
}

impl EnhancePass {
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

    /// Parse structured navigation response from LLM.
    ///
    /// Expected format:
    /// ```text
    /// OVERVIEW: <text>
    /// QUESTIONS: q1, q2, q3
    /// TAGS: tag1, tag2, tag3
    /// ```
    ///
    /// Falls back gracefully: if markers are missing, the entire response
    /// becomes the overview and questions/tags remain empty.
    fn parse_structured_nav_response(response: &str) -> (String, Vec<String>, Vec<String>) {
        let mut overview = String::new();
        let mut questions: Vec<String> = Vec::new();
        let mut tags: Vec<String> = Vec::new();

        for line in response.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("OVERVIEW:") {
                overview = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("QUESTIONS:") {
                questions = rest
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            } else if let Some(rest) = line.strip_prefix("TAGS:") {
                tags = rest
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }

        // Fallback: if no OVERVIEW marker found, use entire response as overview
        if overview.is_empty() {
            overview = response.trim().to_string();
        }

        (overview, questions, tags)
    }

    /// Check if summary generation is needed based on strategy.
    fn needs_summaries(&self, ctx: &CompileContext) -> bool {
        match &ctx.options.summary_strategy {
            SummaryStrategy::None => false,
            SummaryStrategy::Lazy { .. } => false, // Generated on-demand at query time
            SummaryStrategy::Full { .. } | SummaryStrategy::Selective { .. } => true,
        }
    }
}

impl Default for EnhancePass {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CompilePass for EnhancePass {
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

    async fn execute(&mut self, ctx: &mut CompileContext) -> Result<PassResult> {
        let start = Instant::now();

        info!(
            "[enhance] Starting: llm_client={}, strategy={:?}",
            self.llm_client.is_some(),
            ctx.options.summary_strategy
        );

        // Check if we need summaries
        if !self.needs_summaries(ctx) {
            info!(
                "[enhance] Skipped: strategy={:?}",
                ctx.options.summary_strategy
            );
            return Ok(PassResult::success("enhance"));
        }

        // Get LLM client
        let llm_client = match &self.llm_client {
            Some(client) => client,
            None => {
                warn!("[enhance] No LLM client, skipping summary generation");
                return Ok(PassResult::success("enhance"));
            }
        };

        // Get tree
        let tree = match ctx.tree.as_mut() {
            Some(t) => t,
            None => {
                warn!("[enhance] No tree built, skipping");
                return Ok(PassResult::success("enhance"));
            }
        };

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

        // === Incremental: reuse enrichment (summary + keywords + question hints)
        // from the previous tree for nodes whose content is unchanged. Matched by
        // content fingerprint, so it's correct even with duplicate symbol names. ===
        if let Some(ref old_tree) = ctx.existing_tree {
            let index = incremental::build_enrichment_index(old_tree);
            let applied = incremental::apply_enrichment_index(tree, &index);
            for _ in 0..applied {
                ctx.metrics.increment_summaries();
            }
            info!(
                "[enhance] Incremental: reused enrichment for {} of {} nodes",
                applied, total_nodes,
            );
        }

        info!(
            "[enhance] Processing {} nodes for summary generation",
            total_nodes
        );

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
                let content_fp = Fingerprint::from_str(&format!("{}|{}", node.title, node.content));
                let memo_key = MemoKey::summary(&content_fp);
                if let Some(cached) = store
                    .get(&memo_key)
                    .and_then(|c| c.as_summary().map(|s| s.to_string()))
                {
                    if !cached.is_empty() {
                        tree.set_summary(node_id, &cached);
                        debug!(
                            "[enhance] Cache hit: '{}' ({} chars)",
                            node.title,
                            cached.len()
                        );
                        ctx.metrics.increment_summaries();
                        generated += 1;
                        continue;
                    }
                }
            }

            // Shortcut: use original content as summary for short nodes (Borrow A)
            let token_count = node
                .token_count
                .unwrap_or_else(|| vectorless_utils::estimate_tokens(&node.content));
            if shortcut_threshold > 0 && token_count > 0 && token_count <= shortcut_threshold {
                tree.set_summary(node_id, &node.content);
                debug!(
                    "[enhance] Shortcut: '{}' ({} tokens, using original content)",
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
                "[enhance] Generating summaries for {} nodes (concurrency: {})",
                pending_llm.len(),
                concurrency
            );

            // Collect results: (NodeId, is_leaf, Result<String>)
            let results: Vec<(NodeId, bool, std::result::Result<String, String>)> =
                futures::stream::iter(pending_llm)
                    .map(|pending| {
                        let generator = Arc::clone(&generator);
                        async move {
                            let result = generator
                                .generate_for_node(
                                    &pending.title,
                                    &pending.content,
                                    pending.is_leaf,
                                )
                                .await;
                            (
                                pending.node_id,
                                pending.is_leaf,
                                result.map_err(|e| e.to_string()),
                            )
                        }
                    })
                    .buffer_unordered(concurrency)
                    .collect()
                    .await;

            // Write results back to tree
            for (node_id, is_leaf, result) in results {
                ctx.metrics.increment_llm_calls();
                match result {
                    Ok(response) => {
                        if response.is_empty() {
                            failed += 1;
                        } else {
                            ctx.metrics
                                .add_tokens_generated(vectorless_utils::estimate_tokens(&response));

                            if is_leaf {
                                // Leaf node: response is a plain content summary
                                tree.set_summary(node_id, &response);
                            } else {
                                // Non-leaf node: response is structured (OVERVIEW/QUESTIONS/TAGS)
                                let (overview, questions, tags) =
                                    Self::parse_structured_nav_response(&response);
                                tree.set_summary(node_id, &overview);

                                if let Some(node) = tree.get_mut(node_id) {
                                    node.question_hints = questions;
                                    node.routing_keywords = tags;
                                }
                            }
                            generated += 1;
                            ctx.metrics.increment_summaries();
                        }
                    }
                    Err(e) => {
                        warn!("[enhance] LLM summary failed: {}", e);
                        failed += 1;
                    }
                }
            }
        }

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_enhance(duration);
        if failed > 0 {
            ctx.metrics.add_summaries_failed(failed);
        }

        info!(
            "[enhance] Complete: {} summaries ({} shortcut, {} failed, {} no-content, {} skipped-tokens) in {}ms",
            generated, shortcut_used, failed, skipped_no_content, skipped_tokens, duration
        );

        let mut stage_result = PassResult::success("enhance");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_structured_nav_response_full() {
        let response = "\
OVERVIEW: This section covers payment integration and billing configuration.
QUESTIONS: How to set up payments?, What currencies are supported?, How to configure invoices?
TAGS: payments, billing, invoices, currency";

        let (overview, questions, tags) = EnhancePass::parse_structured_nav_response(response);

        assert!(overview.contains("payment integration"));
        assert_eq!(questions.len(), 3);
        assert!(questions[0].contains("set up payments"));
        assert_eq!(tags.len(), 4);
        assert_eq!(tags[0], "payments");
    }

    #[test]
    fn test_parse_structured_nav_response_partial() {
        // Only overview, no questions or tags
        let response = "OVERVIEW: A general introduction to the system.";
        let (overview, questions, tags) = EnhancePass::parse_structured_nav_response(response);

        assert!(overview.contains("general introduction"));
        assert!(questions.is_empty());
        assert!(tags.is_empty());
    }

    #[test]
    fn test_parse_structured_nav_response_fallback() {
        // No markers at all — fallback to entire response as overview
        let response = "This is just a plain summary without any markers.";
        let (overview, questions, tags) = EnhancePass::parse_structured_nav_response(response);

        assert_eq!(overview, response.trim());
        assert!(questions.is_empty());
        assert!(tags.is_empty());
    }

    #[test]
    fn test_parse_structured_nav_response_empty() {
        let (overview, questions, tags) = EnhancePass::parse_structured_nav_response("");
        assert!(overview.is_empty());
        assert!(questions.is_empty());
        assert!(tags.is_empty());
    }
}
