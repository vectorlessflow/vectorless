// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Incremental indexing support.
//!
//! This module provides functionality to incrementally update
//! an existing document index when the source document changes.

use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::core::DocumentTree;

/// Incremental indexer for updating existing indices.
pub struct IncrementalIndexer {
    /// Hash of the last indexed content.
    content_hash: Option<u64>,

    /// Last modification time of the source file.
    last_modified: Option<std::time::SystemTime>,
}

impl IncrementalIndexer {
    /// Create a new incremental indexer.
    pub fn new() -> Self {
        Self {
            content_hash: None,
            last_modified: None,
        }
    }

    /// Check if reindexing is needed.
    pub fn needs_reindex(&self, content: &str) -> bool {
        let new_hash = hash_content(content);
        match self.content_hash {
            Some(old_hash) => old_hash != new_hash,
            None => true,
        }
    }

    /// Update the content hash after indexing.
    pub fn update_hash(&mut self, content: &str) {
        self.content_hash = Some(hash_content(content));
    }

    /// Update the last modified time.
    pub fn update_modified(&mut self) {
        self.last_modified = Some(std::time::SystemTime::now());
    }
}

impl Default for IncrementalIndexer {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash content using DefaultHasher.
fn hash_content(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Diff result between old and new document structure.
#[derive(Debug, Clone)]
pub struct DiffResult {
    /// Nodes that were added.
    pub added: Vec<String>,

    /// Nodes that were removed.
    pub removed: Vec<String>,

    /// Nodes that potentially changed (same title).
    pub common: Vec<String>,
}

/// Compare two document trees and find differences.
pub fn diff_trees(old_tree: &DocumentTree, new_tree: &DocumentTree) -> DiffResult {
    let old_nodes = collect_node_titles(old_tree);
    let new_nodes = collect_node_titles(new_tree);

    let added: Vec<String> = new_nodes
        .difference(&old_nodes)
        .cloned()
        .collect();

    let removed: Vec<String> = old_nodes
        .difference(&new_nodes)
        .cloned()
        .collect();

    let common: Vec<String> = old_nodes
        .intersection(&new_nodes)
        .cloned()
        .collect();

    DiffResult {
        added,
        removed,
        common,
    }
}

/// Collect all node titles from a tree.
fn collect_node_titles(tree: &DocumentTree) -> HashSet<String> {
    let mut titles = HashSet::new();
    for node_id in tree.traverse() {
        if let Some(node) = tree.get(node_id) {
            titles.insert(node.title.clone());
        }
    }
    titles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_reindex() {
        let mut indexer = IncrementalIndexer::new();

        // First time always needs reindex
        assert!(indexer.needs_reindex("content"));

        // After update, same content doesn't need reindex
        indexer.update_hash("content");
        assert!(!indexer.needs_reindex("content"));

        // Different content needs reindex
        assert!(indexer.needs_reindex("different content"));
    }
}
