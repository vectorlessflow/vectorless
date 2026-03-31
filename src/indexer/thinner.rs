// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Tree thinning - merge small nodes with parents.
//!
//! This module implements the "thinning" strategy where nodes
//! with token counts below a threshold are merged with their parent.

use crate::config::IndexerConfig;
use crate::core::{DocumentTree, NodeId};

/// Thin a document tree by merging small nodes.
///
/// Nodes with token count below `subsection_threshold` are merged into their parent.
pub fn thin_tree(tree: &mut DocumentTree, config: &IndexerConfig) {
    // Get all leaf nodes first
    let leaves: Vec<NodeId> = tree.leaves();

    // Process from leaves to root, merging small nodes
    for leaf_id in leaves {
        merge_small_node(tree, leaf_id, config.subsection_threshold);
    }
}

/// Merge a small node with its parent if below threshold.
fn merge_small_node(tree: &mut DocumentTree, node_id: NodeId, min_tokens: usize) {
    // Skip root node
    if node_id == tree.root() {
        return;
    }

    // Get the node's token count
    let token_count = tree.get(node_id)
        .and_then(|n| n.token_count)
        .unwrap_or(0);

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

#[cfg(test)]
mod tests {

    #[test]
    fn test_subtree_token_count() {
        // This would need a test tree setup
    }
}
