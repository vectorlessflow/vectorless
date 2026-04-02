// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Enrich stage - Add metadata to the tree.

use super::async_trait;
use std::time::Instant;
use tracing::info;

use crate::core::{NodeId, Result, VectorlessTree};
use crate::core::toc::TocView;

use super::{IndexStage, StageResult};
use crate::core::index::context::IndexContext;

/// Enrich stage - adds metadata to the tree.
pub struct EnrichStage;

impl EnrichStage {
    /// Create a new enrich stage.
    pub fn new() -> Self {
        Self
    }

    /// Calculate page ranges for all nodes.
    fn calculate_page_ranges(tree: &mut VectorlessTree) {
        // Propagate page ranges up the tree
        Self::propagate_page_ranges(tree, tree.root());
    }

    /// Recursively propagate page ranges from children to parent.
    fn propagate_page_ranges(tree: &mut VectorlessTree, node_id: NodeId) {
        let children = tree.children(node_id);

        if children.is_empty() {
            return;
        }

        // First, propagate to all children
        for child_id in &children {
            Self::propagate_page_ranges(tree, *child_id);
        }

        // Then calculate this node's range from children
        let mut min_page: Option<usize> = None;
        let mut max_page: Option<usize> = None;

        for child_id in &children {
            if let Some(child) = tree.get(*child_id) {
                if let Some(start) = child.start_page {
                    min_page = Some(min_page.map_or(start, |m| m.min(start)));
                }
                if let Some(end) = child.end_page {
                    max_page = Some(max_page.map_or(end, |m| m.max(end)));
                }
            }
        }

        // Update this node's page range
        if let (Some(min), Some(max)) = (min_page, max_page) {
            tree.set_page_boundaries(node_id, min, max);
        }
    }

    /// Calculate token statistics.
    fn calculate_token_stats(tree: &VectorlessTree) -> (usize, usize) {
        let mut total_tokens = 0;
        let mut node_count = 0;

        for node_id in tree.traverse() {
            if let Some(node) = tree.get(node_id) {
                total_tokens += node.token_count.unwrap_or(0);
                node_count += 1;
            }
        }

        (total_tokens, node_count)
    }

    /// Generate document description from root summary.
    fn generate_description(&self, ctx: &mut IndexContext) {
        if !ctx.options.generate_description {
            return;
        }

        // Use root summary if available
        if let Some(tree) = &ctx.tree {
            if let Some(root) = tree.get(tree.root()) {
                if !root.summary.is_empty() {
                    ctx.description = Some(root.summary.clone());
                    info!("Using root summary as document description");
                }
            }
        }
    }
}

impl Default for EnrichStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexStage for EnrichStage {
    fn name(&self) -> &str {
        "enrich"
    }

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        let start = Instant::now();

        let tree = ctx.tree.as_mut().ok_or_else(|| {
            crate::core::Error::IndexBuild("Tree not built".to_string())
        })?;

        // 1. Calculate page ranges
        Self::calculate_page_ranges(tree);
        info!("Calculated page ranges for all nodes");

        // 2. Generate ToC view (cached in context)
        let toc_view = TocView::new();
        let toc = toc_view.generate(tree);
        let _toc_markdown = toc_view.format_markdown(&toc);
        // Could store ToC in context if needed

        // 3. Calculate token statistics
        let (total_tokens, node_count) = Self::calculate_token_stats(tree);
        info!("Total tokens: {}, nodes: {}", total_tokens, node_count);

        // 4. Generate document description
        self.generate_description(ctx);

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_enrich(duration);

        info!("Enriched tree metadata in {}ms", duration);

        let mut stage_result = StageResult::success("enrich");
        stage_result.duration_ms = duration;
        stage_result.metadata.insert(
            "total_tokens".to_string(),
            serde_json::json!(total_tokens),
        );
        stage_result.metadata.insert(
            "node_count".to_string(),
            serde_json::json!(node_count),
        );

        Ok(stage_result)
    }
}
