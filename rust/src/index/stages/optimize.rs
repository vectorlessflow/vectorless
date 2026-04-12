// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Optimize stage - Optimize tree structure.

use super::{AccessPattern, async_trait};
use std::time::Instant;
use tracing::info;

use crate::document::NodeId;
use crate::error::Result;
use crate::index::pipeline::IndexContext;

use super::{IndexStage, StageResult};

/// Optimize stage - optimizes tree structure.
pub struct OptimizeStage;

impl OptimizeStage {
    /// Create a new optimize stage.
    pub fn new() -> Self {
        Self
    }

    /// Merge adjacent small leaf nodes that are siblings under the same parent.
    ///
    /// Only merges nodes that are both **leaves** (no children of their own).
    /// Non-leaf nodes (section headings with subsections) are never merged,
    /// even if their own content is empty.
    fn merge_small_leaves(
        tree: &mut crate::document::DocumentTree,
        min_tokens: usize,
        metrics: &mut crate::index::IndexMetrics,
    ) -> usize {
        let mut merged_count = 0;

        // Get all non-leaf nodes (parents whose children may be candidates)
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

            // Collect children info: only leaf nodes are merge candidates
            let candidates: Vec<(NodeId, usize, bool)> = children
                .iter()
                .map(|&id| {
                    let tokens = tree.get(id).and_then(|n| n.token_count).unwrap_or(0);
                    let is_leaf = tree.is_leaf(id);
                    (id, tokens, is_leaf)
                })
                .collect();

            // Find pairs of adjacent small leaf siblings
            let mut i = 0;
            while i < candidates.len() - 1 {
                let (curr_id, curr_tokens, curr_is_leaf) = candidates[i];
                let (next_id, next_tokens, next_is_leaf) = candidates[i + 1];

                // Both must be leaves with actual content, and both must be small
                if curr_is_leaf
                    && next_is_leaf
                    && curr_tokens > 0
                    && curr_tokens < min_tokens
                    && next_tokens > 0
                    && next_tokens < min_tokens
                {
                    // Merge next into current
                    if let Some(next_node) = tree.get(next_id).cloned() {
                        if let Some(curr) = tree.get_mut(curr_id) {
                            if !next_node.content.is_empty() {
                                if !curr.content.is_empty() {
                                    curr.content.push_str("\n\n");
                                }
                                // Prefix with heading to preserve boundary
                                curr.content
                                    .push_str(&format!("## {}\n{}", next_node.title, next_node.content));
                            }
                            curr.token_count =
                                Some(curr.token_count.unwrap_or(0) + next_tokens);
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

    /// Remove empty intermediate nodes (skip root).
    fn remove_empty_nodes(tree: &mut crate::document::DocumentTree) -> usize {
        let mut removed_count = 0;
        let root = tree.root();

        // Find non-root nodes with no content and only one child
        let candidates: Vec<NodeId> = tree
            .traverse()
            .into_iter()
            .filter(|id| {
                // Skip root node
                if *id == root {
                    return false;
                }
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
    fn name(&self) -> &'static str {
        "optimize"
    }

    fn is_optional(&self) -> bool {
        true
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["enrich"]
    }

    fn access_pattern(&self) -> AccessPattern {
        AccessPattern {
            reads_tree: true,
            writes_tree: true, // merges small leaf nodes
            ..Default::default()
        }
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
            .ok_or_else(|| crate::Error::IndexBuild("Tree not built".to_string()))?;

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
