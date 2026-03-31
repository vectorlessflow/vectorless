// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Node merging utilities.
//!
//! This module provides functionality to merge adjacent nodes
//! when they are too small individually.

use crate::core::{DocumentTree, NodeId};

/// Merge adjacent sibling nodes that are below token threshold.
///
/// This is useful for combining small sections that don't warrant
/// separate nodes in the index.
pub fn merge_adjacent_small_nodes(
    tree: &mut DocumentTree,
    node_id: NodeId,
    min_tokens: usize,
) {
    let children = tree.children(node_id);

    if children.len() < 2 {
        return;
    }

    // Create a list of (node_id, token_count) for processing
    let child_info: Vec<(NodeId, usize)> = children
        .iter()
        .map(|&id| {
            let tokens = tree.get(id)
                .and_then(|n| n.token_count)
                .unwrap_or(0);
            (id, tokens)
        })
        .collect();

    // Find pairs of adjacent small nodes to merge
    let mut to_merge: Vec<(NodeId, NodeId)> = Vec::new();
    let mut i = 0;
    while i < child_info.len() - 1 {
        let (curr_id, curr_tokens) = child_info[i];
        let (next_id, next_tokens) = child_info[i + 1];

        // If both are small, mark for merging
        if curr_tokens < min_tokens && next_tokens < min_tokens {
            to_merge.push((curr_id, next_id));
            i += 2; // Skip the merged node
        } else {
            i += 1;
        }
    }

    // Perform merges
    for (curr_id, next_id) in to_merge {
        merge_next_into_current(tree, curr_id, next_id);
    }
}

/// Merge next node into current node.
fn merge_next_into_current(tree: &mut DocumentTree, curr_id: NodeId, next_id: NodeId) {
    // Get next node data
    let next_data = tree.get(next_id).cloned();

    if let Some(next_node) = next_data {
        if let Some(curr) = tree.get_mut(curr_id) {
            // Append content
            if !next_node.content.is_empty() {
                if !curr.content.is_empty() {
                    curr.content.push('\n');
                }
                curr.content.push_str(&next_node.content);
            }

            // Add token count
            let curr_tokens = curr.token_count.unwrap_or(0);
            let next_tokens = next_node.token_count.unwrap_or(0);
            curr.token_count = Some(curr_tokens + next_tokens);

            // Update end_page
            if let Some(end_page) = next_node.end_page {
                curr.end_page = Some(end_page);
            }
        }
    }

    // Mark next as merged
    if let Some(node) = tree.get_mut(next_id) {
        node.title = format!("[MERGED: {}]", node.title);
        node.content.clear();
        node.token_count = Some(0);
    }
}

/// Merge all small children into parent.
pub fn merge_children_into_parent(tree: &mut DocumentTree, parent_id: NodeId, min_tokens: usize) {
    let children = tree.children(parent_id);

    if children.is_empty() {
        return;
    }

    // Calculate total tokens of all children
    let total_child_tokens: usize = children
        .iter()
        .filter_map(|&id| tree.get(id).and_then(|n| n.token_count))
        .sum();

    // If total is below threshold, merge all into parent
    if total_child_tokens < min_tokens {
        let mut merged_content = String::new();
        let mut total_tokens = 0;
        let mut end_page = None;

        for &child_id in &children {
            if let Some(child) = tree.get(child_id) {
                if !child.content.is_empty() {
                    if !merged_content.is_empty() {
                        merged_content.push('\n');
                    }
                    merged_content.push_str(&child.content);
                }
                total_tokens += child.token_count.unwrap_or(0);
                if let Some(ep) = child.end_page {
                    end_page = Some(ep);
                }
            }
        }

        if let Some(parent) = tree.get_mut(parent_id) {
            if !merged_content.is_empty() {
                if !parent.content.is_empty() {
                    parent.content.push('\n');
                }
                parent.content.push_str(&merged_content);
            }
            let parent_tokens = parent.token_count.unwrap_or(0);
            parent.token_count = Some(parent_tokens + total_tokens);
            if end_page.is_some() {
                parent.end_page = end_page;
            }
        }

        // Mark children as merged
        for &child_id in &children {
            if let Some(node) = tree.get_mut(child_id) {
                node.title = format!("[MERGED: {}]", node.title);
                node.content.clear();
                node.token_count = Some(0);
            }
        }
    }
}
