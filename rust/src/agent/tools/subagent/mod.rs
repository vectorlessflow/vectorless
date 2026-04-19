// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! SubAgent tools: ls, cd, cd_up, cat, pwd, grep, head, find_tree, wc.

mod cat;
mod cd;
mod find;
mod grep;
mod head;
mod ls;
mod pwd;
mod wc;

pub use cat::cat;
pub use cd::{cd, cd_up};
pub use find::find_tree;
pub use grep::grep;
pub use head::head;
pub use ls::ls;
pub use pwd::pwd;
pub use wc::wc;

use crate::document::{DocumentTree, NodeId};

/// Collect all NodeIds in the subtree rooted at `node` (inclusive).
pub(super) fn collect_subtree(node: NodeId, tree: &DocumentTree) -> Vec<NodeId> {
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
