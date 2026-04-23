// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Optimize stage - Optimize tree structure.

use super::{AccessPattern, async_trait};
use std::time::Instant;
use tracing::{debug, info};

use vectorless_document::NodeId;
use vectorless_error::Result;
use crate::pipeline::IndexContext;

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
        tree: &mut vectorless_document::DocumentTree,
        min_tokens: usize,
        metrics: &mut crate::IndexMetrics,
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
                                curr.content.push_str(&format!(
                                    "## {}\n{}",
                                    next_node.title, next_node.content
                                ));
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

    /// Remove empty intermediate nodes (skip root).
    fn remove_empty_nodes(tree: &mut vectorless_document::DocumentTree) -> usize {
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
        vec!["enrich", "navigation_index"]
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
            debug!("[optimize] Disabled, skipping");
            return Ok(StageResult::success("optimize"));
        }

        let tree = ctx
            .tree
            .as_mut()
            .ok_or_else(|| vectorless_error::Error::IndexBuild("Tree not built".to_string()))?;

        let node_count = tree.node_count();
        info!(
            "[optimize] Starting: {} nodes, merge_threshold={}",
            node_count, config.merge_leaf_threshold,
        );

        let mut merged_count = 0;

        // 1. Merge small leaves
        if config.merge_leaf_threshold > 0 {
            merged_count =
                Self::merge_small_leaves(tree, config.merge_leaf_threshold, &mut ctx.metrics);
            if merged_count > 0 {
                debug!("[optimize] Merged {} small leaf nodes", merged_count);
            }
        }

        // 2. Remove empty intermediate nodes
        let removed_count = Self::remove_empty_nodes(tree);
        if removed_count > 0 {
            debug!(
                "[optimize] Marked {} empty intermediate nodes",
                removed_count
            );
        }

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_optimize(duration);

        info!(
            "[optimize] Complete: {} merged, {} emptied in {}ms",
            merged_count, removed_count, duration
        );

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

#[cfg(test)]
mod tests {
    use super::*;
    use vectorless_document::DocumentTree;
    use crate::PipelineOptions;
    use crate::pipeline::IndexContext;
    use crate::pipeline::IndexInput;

    /// Create a tree with small leaf children under root for merge tests.
    ///
    /// ```text
    /// Root
    /// ├── Leaf A (50 tokens)
    /// ├── Leaf B (30 tokens)   ← should merge with Leaf A
    /// ├── Leaf C (200 tokens)  ← too large, not merged
    /// └── Leaf D (40 tokens)   ← no adjacent small sibling
    /// ```
    fn make_merge_test_tree() -> DocumentTree {
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();

        let a = tree.add_child(root, "Leaf A", "content A");
        let b = tree.add_child(root, "Leaf B", "content B");
        let c = tree.add_child(root, "Leaf C", "content C long");
        let d = tree.add_child(root, "Leaf D", "content D");

        // Set token counts
        if let Some(n) = tree.get_mut(a) {
            n.token_count = Some(50);
        }
        if let Some(n) = tree.get_mut(b) {
            n.token_count = Some(30);
        }
        if let Some(n) = tree.get_mut(c) {
            n.token_count = Some(200);
        }
        if let Some(n) = tree.get_mut(d) {
            n.token_count = Some(40);
        }

        tree
    }

    #[test]
    fn test_merge_small_leaves_merges_adjacent_pair() {
        let mut tree = make_merge_test_tree();
        let root = tree.root();
        let mut metrics = crate::pipeline::IndexMetrics::new();

        // Threshold 100: Leaf A (50) and Leaf B (30) should merge
        let merged = OptimizeStage::merge_small_leaves(&mut tree, 100, &mut metrics);

        assert_eq!(merged, 1);
        assert_eq!(metrics.nodes_merged, 1);

        // Leaf B should be marked as merged
        let children = tree.children(root);
        let leaf_b = children.iter().find(|&&id| {
            tree.get(id)
                .map(|n| n.title.starts_with("[MERGED"))
                .unwrap_or(false)
        });
        assert!(leaf_b.is_some(), "Leaf B should be marked as merged");
    }

    #[test]
    fn test_merge_small_leaves_nothing_above_threshold() {
        let mut tree = make_merge_test_tree();
        let mut metrics = crate::pipeline::IndexMetrics::new();

        // Threshold 10: all leaves are above this, nothing merges
        let merged = OptimizeStage::merge_small_leaves(&mut tree, 10, &mut metrics);
        assert_eq!(merged, 0);
    }

    #[test]
    fn test_merge_small_leaves_preserves_content() {
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let a = tree.add_child(root, "A", "hello");
        let b = tree.add_child(root, "B", "world");
        if let Some(n) = tree.get_mut(a) {
            n.token_count = Some(5);
        }
        if let Some(n) = tree.get_mut(b) {
            n.token_count = Some(5);
        }

        let mut metrics = crate::pipeline::IndexMetrics::new();
        let _ = OptimizeStage::merge_small_leaves(&mut tree, 100, &mut metrics);

        // Leaf A should now contain both contents with heading prefix
        let a_node = tree.get(a).unwrap();
        assert!(a_node.content.contains("hello"));
        assert!(a_node.content.contains("## B"));
        assert!(a_node.content.contains("world"));
        assert_eq!(a_node.token_count, Some(10));
    }

    #[test]
    fn test_merge_small_leaves_skips_non_leaf() {
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();

        // Section is a non-leaf (has a child), should not be merged even if small
        let section = tree.add_child(root, "Section", "section content");
        let _sub = tree.add_child(section, "Sub", "sub content");
        let leaf = tree.add_child(root, "Leaf", "leaf content");

        if let Some(n) = tree.get_mut(section) {
            n.token_count = Some(5);
        }
        if let Some(n) = tree.get_mut(leaf) {
            n.token_count = Some(5);
        }

        let mut metrics = crate::pipeline::IndexMetrics::new();
        let merged = OptimizeStage::merge_small_leaves(&mut tree, 100, &mut metrics);

        // Section is non-leaf, only Leaf is a leaf — no adjacent pair of leaves
        assert_eq!(merged, 0);
    }

    #[test]
    fn test_remove_empty_nodes_marks_single_child_empty() {
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();

        // Empty intermediate with single child
        let section = tree.add_child(root, "Section", "");
        let _leaf = tree.add_child(section, "Leaf", "content");

        let removed = OptimizeStage::remove_empty_nodes(&mut tree);
        assert_eq!(removed, 1);

        let section_node = tree.get(section).unwrap();
        assert!(section_node.title.starts_with("[EMPTY"));
    }

    #[test]
    fn test_remove_empty_nodes_skips_root() {
        let mut tree = DocumentTree::new("Root", "");
        let _child = tree.add_child(tree.root(), "Child", "content");

        let removed = OptimizeStage::remove_empty_nodes(&mut tree);
        assert_eq!(removed, 0);
    }

    #[test]
    fn test_remove_empty_nodes_skips_leaves() {
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let leaf = tree.add_child(root, "Leaf", "");

        let removed = OptimizeStage::remove_empty_nodes(&mut tree);
        assert_eq!(removed, 0, "Leaves should not be removed");

        // Verify the leaf is indeed a leaf
        assert!(tree.is_leaf(leaf));
    }

    #[test]
    fn test_remove_empty_nodes_skips_multi_child() {
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let section = tree.add_child(root, "Section", "");
        let _c1 = tree.add_child(section, "C1", "a");
        let _c2 = tree.add_child(section, "C2", "b");

        let removed = OptimizeStage::remove_empty_nodes(&mut tree);
        assert_eq!(
            removed, 0,
            "Nodes with multiple children should not be removed"
        );
    }

    #[test]
    fn test_remove_empty_nodes_skips_non_empty() {
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let section = tree.add_child(root, "Section", "has content");
        let _leaf = tree.add_child(section, "Leaf", "content");

        let removed = OptimizeStage::remove_empty_nodes(&mut tree);
        assert_eq!(removed, 0);
    }

    #[tokio::test]
    async fn test_optimize_disabled_skips() {
        let mut stage = OptimizeStage::new();
        assert_eq!(stage.name(), "optimize");
        assert!(stage.is_optional());
        assert_eq!(stage.depends_on(), vec!["enrich", "navigation_index"]);

        let mut options = PipelineOptions::default();
        options.optimization.enabled = false;

        let input = IndexInput::content("# Test\nHello");
        let mut ctx = IndexContext::new(input, options);
        ctx.tree = Some(DocumentTree::new("Root", "content"));

        let result = stage.execute(&mut ctx).await.unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_merge_small_leaves_empty_tree() {
        let mut tree = DocumentTree::new("Root", "");
        let mut metrics = crate::pipeline::IndexMetrics::new();

        let merged = OptimizeStage::merge_small_leaves(&mut tree, 100, &mut metrics);
        assert_eq!(merged, 0, "Root with no children should merge nothing");
    }
}
