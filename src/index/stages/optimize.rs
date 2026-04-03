// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Optimize stage - Optimize tree structure.

use super::async_trait;
use std::time::Instant;
use tracing::info;

use crate::domain::{NodeId, Result};
use crate::index::pipeline::IndexContext;

use super::{IndexStage, StageResult};

/// Optimize stage - optimizes tree structure.
pub struct OptimizeStage;

impl OptimizeStage {
    /// Create a new optimize stage.
    pub fn new() -> Self {
        Self
    }

    /// Merge adjacent small leaf nodes.
    fn merge_small_leaves(
        tree: &mut crate::domain::DocumentTree,
        min_tokens: usize,
        metrics: &mut crate::index::IndexMetrics,
    ) -> usize {
        let mut merged_count = 0;

        // Get all non-leaf nodes
        let non_leaves: Vec<NodeId> = tree
            .traverse()
            .into_iter()
            .filter(|id| !tree.is_leaf(*id))
            .collect();

        for parent_id in non_leaves {
            let children = tree.children(parent_id);
            if children.len() < 2 {
                continue;
            }

            // Find pairs of adjacent small nodes
            let mut i = 0;
            while i < children.len() - 1 {
                let curr_id = children[i];
                let next_id = children[i + 1];

                let curr_tokens = tree.get(curr_id).and_then(|n| n.token_count).unwrap_or(0);
                let next_tokens = tree.get(next_id).and_then(|n| n.token_count).unwrap_or(0);

                // If both are small, merge next into current
                if curr_tokens < min_tokens && next_tokens < min_tokens {
                    // Merge content
                    if let Some(next_node) = tree.get(next_id).cloned() {
                        if let Some(curr) = tree.get_mut(curr_id) {
                            if !next_node.content.is_empty() {
                                if !curr.content.is_empty() {
                                    curr.content.push('\n');
                                }
                                curr.content.push_str(&next_node.content);
                            }
                            curr.token_count = Some(curr.token_count.unwrap_or(0) + next_tokens);
                        }
                    }

                    // Mark next as merged
                    if let Some(node) = tree.get_mut(next_id) {
                        node.title = format!("[MERGED: {}]", node.title);
                        node.content.clear();
                        node.token_count = Some(0);
                    }

                    merged_count += 1;
                    metrics.increment_nodes_merged();
                    i += 2; // Skip merged node
                } else {
                    i += 1;
                }
            }
        }

        merged_count
    }

    /// Remove empty intermediate nodes.
    fn remove_empty_nodes(tree: &mut crate::domain::DocumentTree) -> usize {
        let mut removed_count = 0;

        // Find nodes with no content and only one child
        let candidates: Vec<NodeId> = tree
            .traverse()
            .into_iter()
            .filter(|id| {
                if tree.is_leaf(*id) {
                    return false;
                }
                let children = tree.children(*id);
                if children.len() != 1 {
                    return false;
                }
                if let Some(node) = tree.get(*id) {
                    node.content.trim().is_empty()
                } else {
                    false
                }
            })
            .collect();

        // Note: Actually removing nodes from arena tree is complex
        // For now, we just mark them
        for node_id in candidates {
            if let Some(node) = tree.get_mut(node_id) {
                node.title = format!("[EMPTY: {}]", node.title);
                removed_count += 1;
            }
        }

        removed_count
    }
}

impl Default for OptimizeStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexStage for OptimizeStage {
    fn name(&self) -> &str {
        "optimize"
    }

    fn is_optional(&self) -> bool {
        true
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["enrich"]
    }

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        let start = Instant::now();

        let config = &ctx.options.optimization;
        if !config.enabled {
            info!("Tree optimization disabled, skipping");
            return Ok(StageResult::success("optimize"));
        }

        let tree = ctx
            .tree
            .as_mut()
            .ok_or_else(|| crate::domain::Error::IndexBuild("Tree not built".to_string()))?;

        let mut merged_count = 0;

        // 1. Merge small leaves
        if config.merge_leaf_threshold > 0 {
            merged_count =
                Self::merge_small_leaves(tree, config.merge_leaf_threshold, &mut ctx.metrics);
            info!("Merged {} small leaf nodes", merged_count);
        }

        // 2. Remove empty intermediate nodes
        let removed_count = Self::remove_empty_nodes(tree);
        if removed_count > 0 {
            info!("Marked {} empty intermediate nodes", removed_count);
        }

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_optimize(duration);

        info!("Optimized tree in {}ms", duration);

        let mut stage_result = StageResult::success("optimize");
        stage_result.duration_ms = duration;
        stage_result
            .metadata
            .insert("nodes_merged".to_string(), serde_json::json!(merged_count));
        stage_result.metadata.insert(
            "nodes_removed".to_string(),
            serde_json::json!(removed_count),
        );

        Ok(stage_result)
    }
}
