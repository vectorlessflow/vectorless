// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Incremental indexing support.
//!
//! This module provides functionality to incrementally update
//! an existing document index when the source document changes.
//!
//! # Features
//!
//! - **Fine-grained change detection**: Uses subtree fingerprints to identify
//!   exactly which nodes changed
//! - **Processing version tracking**: Automatically reprocesses when algorithm
//!   versions change
//! - **Partial updates**: Only reprocess changed nodes

mod detector;
mod resolver;
mod updater;

pub use detector::ChangeDetector;
pub use resolver::{IndexAction, SkipInfo, resolve_action};
use std::collections::HashMap;
use vectorless_document::DocumentTree;

/// Reuse summaries from old tree for unchanged nodes in the new tree.
///
/// Uses `ChangeDetector` to find which nodes changed, then copies
/// summaries from old tree nodes with matching titles that are unchanged.
///
/// Returns a map of `title -> summary` for reusable summaries.
pub fn compute_reusable_summaries(
    old_tree: &DocumentTree,
    new_tree: &DocumentTree,
) -> HashMap<String, String> {
    let detector = ChangeDetector::new();
    let changes = detector.detect_changes(old_tree, new_tree);

    let changed_titles: std::collections::HashSet<String> = changes
        .modified
        .iter()
        .chain(changes.restructured.iter())
        .chain(changes.added.iter())
        .chain(changes.removed.iter())
        .map(|c| c.title.clone())
        .collect();

    let mut reusable = HashMap::new();
    for node_id in old_tree.traverse() {
        if let Some(node) = old_tree.get(node_id) {
            if !changed_titles.contains(&node.title) && !node.summary.is_empty() {
                reusable.insert(node.title.clone(), node.summary.clone());
            }
        }
    }
    reusable
}

/// Apply reusable summaries to a new tree.
///
/// For each node in `new_tree` whose title matches a key in `summaries`,
/// sets the node's summary from the map.
///
/// Returns the number of summaries applied.
pub fn apply_reusable_summaries(
    new_tree: &mut DocumentTree,
    summaries: &HashMap<String, String>,
) -> usize {
    let mut applied = 0;
    for node_id in new_tree.traverse() {
        if let Some(node) = new_tree.get(node_id) {
            if node.summary.is_empty() {
                if let Some(summary) = summaries.get(&node.title) {
                    new_tree.set_summary(node_id, summary);
                    applied += 1;
                }
            }
        }
    }
    applied
}
