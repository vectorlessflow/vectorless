// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Subtree traversal helpers.

use vectorless_document::{DocumentTree, NodeId};

/// Collect all NodeIds in the subtree rooted at `node` (inclusive), via DFS.
pub fn collect_subtree(node: NodeId, tree: &DocumentTree) -> Vec<NodeId> {
    let mut result = vec![node];
    let mut stack = vec![node];

    while let Some(current) = stack.pop() {
        for child in tree.children_iter(current) {
            result.push(child);
            stack.push(child);
        }
    }

    result
}
