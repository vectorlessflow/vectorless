// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Tree thinning - merge small nodes with parents.
//!
//! This module implements the "thinning" strategy where nodes
//! with token counts below a threshold are merged with their parent.
//!
//! # Strategy
//!
//! Thinning can be applied at two stages:
//! 1. **Pre-build thinning** (`thin_raw_nodes`) - Applied to raw nodes before tree construction
//! 2. **Post-build thinning** (`thin_tree`) - Applied to an already built tree
//!
//! Pre-build thinning is more efficient as it avoids creating nodes that will be merged.
//!
//! # Example
//!
//! ```rust
//! use vectorless::indexer::{ThinningConfig, thin_raw_nodes, calculate_total_tokens};
//! use vectorless::document::RawNode;
//!
//! let mut nodes = vec![
//!     RawNode { level: 1, title: "Section".into(), content: "Content".into(), ..Default::default() },
//!     RawNode { level: 2, title: "Small".into(), content: "Hi".into(), ..Default::default() },
//! ];
//!
//! // Calculate recursive token counts
//! calculate_total_tokens(&mut nodes);
//!
//! // Apply thinning
//! let config = ThinningConfig::enabled(500);
//! thin_raw_nodes(&mut nodes, &config);
//! ```

use crate::core::{DocumentTree, NodeId};
use crate::document::RawNode;

// ============================================================
// Configuration
// ============================================================

/// Configuration for tree thinning.
#[derive(Debug, Clone)]
pub struct ThinningConfig {
    /// Whether thinning is enabled.
    pub enabled: bool,

    /// Token threshold: nodes with fewer tokens are merged into parent.
    pub threshold: usize,
}

impl Default for ThinningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: 5000,
        }
    }
}

impl ThinningConfig {
    /// Create a disabled config.
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Create an enabled config with the given threshold.
    pub fn enabled(threshold: usize) -> Self {
        Self {
            enabled: true,
            threshold,
        }
    }

    /// Create a builder for ThinningConfig.
    pub fn builder() -> ThinningConfigBuilder {
        ThinningConfigBuilder::default()
    }
}

/// Builder for ThinningConfig.
#[derive(Debug, Clone, Default)]
pub struct ThinningConfigBuilder {
    config: ThinningConfig,
}

impl ThinningConfigBuilder {
    /// Enable thinning.
    pub fn enabled(mut self, value: bool) -> Self {
        self.config.enabled = value;
        self
    }

    /// Set the threshold.
    pub fn threshold(mut self, value: usize) -> Self {
        self.config.threshold = value;
        self
    }

    /// Build the config.
    pub fn build(self) -> ThinningConfig {
        self.config
    }
}

// ============================================================
// Token Counting
// ============================================================

/// Calculate total token counts for all nodes (recursive, includes children).
///
/// This must be called before `thin_raw_nodes` to ensure correct threshold checks.
///
/// # Algorithm
///
/// Process nodes from back to front to ensure children are processed before parents.
/// For each node, total_token_count = own_tokens + sum(children's total_token_count).
pub fn calculate_total_tokens(nodes: &mut [RawNode]) {
    if nodes.is_empty() {
        return;
    }

    // Process from back to front
    for i in (0..nodes.len()).rev() {
        let own_tokens = nodes[i].token_count.unwrap_or_else(|| estimate_tokens(&nodes[i].content));
        nodes[i].token_count = Some(own_tokens);

        // Find all children (direct and indirect)
        let children_tokens: usize = find_all_children_indices(i, nodes)
            .iter()
            .map(|&child_idx| nodes[child_idx].total_token_count.unwrap_or(0))
            .sum();

        nodes[i].total_token_count = Some(own_tokens + children_tokens);
    }
}

/// Find all children (direct and indirect) of a node.
///
/// Children are nodes that:
/// 1. Come after the parent in the list
/// 2. Have a higher level than the parent
/// 3. Are before any node with level <= parent's level
fn find_all_children_indices(parent_idx: usize, nodes: &[RawNode]) -> Vec<usize> {
    let parent_level = nodes[parent_idx].level;
    let mut children = Vec::new();

    for i in (parent_idx + 1)..nodes.len() {
        if nodes[i].level <= parent_level {
            break; // Encountered same or higher level, stop
        }
        children.push(i);
    }

    children
}

/// Find direct children of a node (immediate descendants only).
fn find_direct_children_indices(parent_idx: usize, nodes: &[RawNode]) -> Vec<usize> {
    let parent_level = nodes[parent_idx].level;
    let target_level = parent_level + 1;
    let mut children = Vec::new();
    let mut i = parent_idx + 1;

    while i < nodes.len() {
        if nodes[i].level <= parent_level {
            break; // Encountered same or higher level, stop
        }
        if nodes[i].level == target_level {
            children.push(i);
        }
        i += 1;
    }

    children
}

/// Estimate token count (1 token ≈ 4 characters).
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    (text.len() / 4).max(1)
}

// ============================================================
// Pre-build Thinning (on RawNode slice)
// ============================================================

/// Apply thinning to raw nodes before tree construction.
///
/// Nodes with total_token_count < threshold are marked for merging.
/// The merging is done by the TreeBuilder by not creating tree nodes for them.
///
/// Returns a vector of booleans indicating which nodes should be kept.
pub fn thin_raw_nodes(nodes: &[RawNode], config: &ThinningConfig) -> Vec<bool> {
    if !config.enabled || nodes.is_empty() {
        // Keep all nodes if thinning is disabled
        return vec![true; nodes.len()];
    }

    let mut keep = vec![true; nodes.len()];

    // Process from leaves to root (back to front)
    for i in (0..nodes.len()).rev() {
        let total_tokens = nodes[i].total_token_count.unwrap_or(0);

        // Check if this node should be merged
        if total_tokens < config.threshold {
            // Mark this node for merging (don't keep it)
            keep[i] = false;

            // Note: Content is already in parent's text because
            // the parser extracts text from header to next same/higher level header
        }
    }

    // Ensure we keep at least one node at each direct-child level under each parent
    ensure_min_children(nodes, &mut keep);

    keep
}

/// Ensure each parent keeps at least one direct child.
fn ensure_min_children(nodes: &[RawNode], keep: &mut [bool]) {
    // For each potential parent, ensure at least one child is kept
    for i in 0..nodes.len() {
        let children = find_direct_children_indices(i, nodes);

        if !children.is_empty() {
            // Check if any child is kept
            let has_kept_child = children.iter().any(|&c| keep[c]);

            if !has_kept_child {
                // Keep the child with the most content
                let best_child = children
                    .iter()
                    .max_by_key(|&&c| nodes[c].total_token_count.unwrap_or(0))
                    .copied();

                if let Some(idx) = best_child {
                    keep[idx] = true;
                }
            }
        }
    }
}

// ============================================================
// Post-build Thinning (on DocumentTree)
// ============================================================

/// Thin a document tree by merging small nodes.
///
/// This is a post-build operation that marks small nodes for merging.
/// The merged nodes are not removed but marked with `[MERGED: ...]` prefix.
///
/// For better efficiency, use `thin_raw_nodes` before tree construction.
pub fn thin_tree(tree: &mut DocumentTree, config: &ThinningConfig) {
    if !config.enabled {
        return;
    }

    // Get all leaf nodes first
    let leaves: Vec<NodeId> = tree.leaves();

    // Process from leaves to root, merging small nodes
    for leaf_id in leaves {
        merge_small_node(tree, leaf_id, config.threshold);
    }
}

/// Merge a small node with its parent if below threshold.
fn merge_small_node(tree: &mut DocumentTree, node_id: NodeId, min_tokens: usize) {
    // Skip root node
    if node_id == tree.root() {
        return;
    }

    // Get the subtree token count
    let token_count = subtree_token_count(tree, node_id);

    // Check if node is below threshold
    if token_count >= min_tokens {
        return;
    }

    // Get parent
    let parent_id = match tree.parent(node_id) {
        Some(id) => id,
        None => return,
    };

    // Get node data before mutation
    let (content, end_page) = tree.get(node_id)
        .map(|n| (n.content.clone(), n.end_page))
        .unwrap_or((String::new(), None));

    // Merge node content into parent
    if let Some(parent) = tree.get_mut(parent_id) {
        // Append content to parent
        if !content.is_empty() {
            if !parent.content.is_empty() {
                parent.content.push('\n');
            }
            parent.content.push_str(&content);
        }

        // Add token count to parent
        let parent_tokens = parent.token_count.unwrap_or(0);
        parent.token_count = Some(parent_tokens + token_count);

        // Update end_page if needed
        if let Some(ep) = end_page {
            parent.end_page = Some(ep);
        }
    }

    // Mark the node as merged
    if let Some(node) = tree.get_mut(node_id) {
        node.title = format!("[MERGED: {}]", node.title);
        node.content.clear();
        node.token_count = Some(0);
    }
}

/// Calculate total token count for a subtree.
pub fn subtree_token_count(tree: &DocumentTree, node_id: NodeId) -> usize {
    let node = match tree.get(node_id) {
        Some(n) => n,
        None => return 0,
    };

    let mut total = node.token_count.unwrap_or(0);
    for child_id in tree.children(node_id) {
        total += subtree_token_count(tree, child_id);
    }
    total
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("hi"), 1);
        assert_eq!(estimate_tokens("hello world"), 1);
        assert_eq!(estimate_tokens(&"a".repeat(100)), 25);
    }

    #[test]
    fn test_calculate_total_tokens() {
        let mut nodes = vec![
            RawNode { level: 1, title: "A".into(), content: "12345678".into(), ..Default::default() }, // 2 tokens
            RawNode { level: 2, title: "B".into(), content: "1234".into(), ..Default::default() },     // 1 token
            RawNode { level: 2, title: "C".into(), content: "12".into(), ..Default::default() },       // 1 token
            RawNode { level: 1, title: "D".into(), content: "1234".into(), ..Default::default() },     // 1 token
        ];

        calculate_total_tokens(&mut nodes);

        // Node B: own=1, total=1
        assert_eq!(nodes[1].total_token_count, Some(1));
        // Node C: own=1, total=1
        assert_eq!(nodes[2].total_token_count, Some(1));
        // Node A: own=2, children=2, total=4
        assert_eq!(nodes[0].total_token_count, Some(4));
        // Node D: own=1, total=1
        assert_eq!(nodes[3].total_token_count, Some(1));
    }

    #[test]
    fn test_find_all_children() {
        let nodes = vec![
            RawNode { level: 1, title: "A".into(), ..Default::default() },
            RawNode { level: 2, title: "B".into(), ..Default::default() },
            RawNode { level: 3, title: "C".into(), ..Default::default() },
            RawNode { level: 2, title: "D".into(), ..Default::default() },
            RawNode { level: 1, title: "E".into(), ..Default::default() },
        ];

        // A's children: B, C, D (all nodes with level > 1 before E)
        let children = find_all_children_indices(0, &nodes);
        assert_eq!(children, vec![1, 2, 3]);

        // B's children: C
        let children = find_all_children_indices(1, &nodes);
        assert_eq!(children, vec![2]);

        // E's children: none
        let children = find_all_children_indices(4, &nodes);
        assert!(children.is_empty());
    }

    #[test]
    fn test_find_direct_children() {
        let nodes = vec![
            RawNode { level: 1, title: "A".into(), ..Default::default() },
            RawNode { level: 2, title: "B".into(), ..Default::default() },
            RawNode { level: 3, title: "C".into(), ..Default::default() },
            RawNode { level: 2, title: "D".into(), ..Default::default() },
            RawNode { level: 1, title: "E".into(), ..Default::default() },
        ];

        // A's direct children: B, D (level 2 nodes)
        let children = find_direct_children_indices(0, &nodes);
        assert_eq!(children, vec![1, 3]);

        // B's direct children: C (level 3)
        let children = find_direct_children_indices(1, &nodes);
        assert_eq!(children, vec![2]);
    }

    #[test]
    fn test_thin_raw_nodes_disabled() {
        let nodes = vec![
            RawNode { level: 1, title: "A".into(), content: "x".into(), ..Default::default() },
        ];

        let config = ThinningConfig::disabled();
        let keep = thin_raw_nodes(&nodes, &config);

        assert!(keep[0]);
    }

    #[test]
    fn test_thin_raw_nodes_small_node_merged() {
        let mut nodes = vec![
            RawNode { level: 1, title: "A".into(), content: "xxxxx".into(), ..Default::default() }, // kept
            RawNode { level: 2, title: "B".into(), content: "x".into(), ..Default::default() },     // small, merged
            RawNode { level: 1, title: "C".into(), content: "xxxxx".into(), ..Default::default() }, // kept
        ];

        calculate_total_tokens(&mut nodes);

        let config = ThinningConfig::enabled(100); // threshold = 100 tokens
        let keep = thin_raw_nodes(&nodes, &config);

        // All nodes have < 100 tokens, but ensure_min_children keeps at least one
        assert!(keep[0]); // A is kept
        assert!(keep[2]); // C is kept
    }

    #[test]
    fn test_thinning_config_builder() {
        let config = ThinningConfig::builder()
            .enabled(true)
            .threshold(3000)
            .build();

        assert!(config.enabled);
        assert_eq!(config.threshold, 3000);
    }
}
