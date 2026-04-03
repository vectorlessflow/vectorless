// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Change detection for incremental updates.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::SystemTime;

use crate::domain::VectorlessTree;

/// Type of change detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    /// Node was added.
    Added,
    /// Node was removed.
    Removed,
    /// Node content changed.
    Modified,
    /// Node structure changed (children added/removed).
    Restructured,
}

/// A single change in the document.
#[derive(Debug, Clone)]
pub struct NodeChange {
    /// Node ID (from old tree).
    pub node_id: Option<String>,
    /// Node title.
    pub title: String,
    /// Type of change.
    pub change_type: ChangeType,
}

/// Set of changes between two document versions.
#[derive(Debug, Clone, Default)]
pub struct ChangeSet {
    /// Added nodes.
    pub added: Vec<NodeChange>,
    /// Removed nodes.
    pub removed: Vec<NodeChange>,
    /// Modified nodes.
    pub modified: Vec<NodeChange>,
    /// Restructured nodes.
    pub restructured: Vec<NodeChange>,
}

impl ChangeSet {
    /// Create an empty change set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are any changes.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.modified.is_empty()
            && self.restructured.is_empty()
    }

    /// Get total number of changes.
    pub fn total_changes(&self) -> usize {
        self.added.len()
            + self.removed.len()
            + self.modified.len()
            + self.restructured.len()
    }

    /// Merge another change set into this one.
    pub fn merge(&mut self, other: ChangeSet) {
        self.added.extend(other.added);
        self.removed.extend(other.removed);
        self.modified.extend(other.modified);
        self.restructured.extend(other.restructured);
    }
}

/// Change detector for incremental updates.
pub struct ChangeDetector {
    /// Content hashes by document ID.
    hashes: HashMap<String, u64>,

    /// File modification times by document ID.
    mtimes: HashMap<String, SystemTime>,
}

impl ChangeDetector {
    /// Create a new change detector.
    pub fn new() -> Self {
        Self {
            hashes: HashMap::new(),
            mtimes: HashMap::new(),
        }
    }

    /// Compute hash of content.
    fn hash_content(content: &str) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Check if a file needs reindexing based on mtime.
    pub fn needs_reindex_by_mtime(&self, doc_id: &str, path: &Path) -> bool {
        // Check if we have recorded mtime
        let Some(recorded_mtime) = self.mtimes.get(doc_id) else {
            return true; // Never indexed
        };

        // Get current mtime
        let Ok(metadata) = std::fs::metadata(path) else {
            return true; // Can't read file
        };

        let Ok(current_mtime) = metadata.modified() else {
            return true;
        };

        current_mtime > *recorded_mtime
    }

    /// Check if content needs reindexing based on hash.
    pub fn needs_reindex_by_hash(&self, doc_id: &str, content: &str) -> bool {
        let current_hash = Self::hash_content(content);

        match self.hashes.get(doc_id) {
            Some(recorded_hash) => *recorded_hash != current_hash,
            None => true,
        }
    }

    /// Record content hash and mtime for a document.
    pub fn record(&mut self, doc_id: &str, content: &str, path: Option<&Path>) {
        // Record hash
        let hash = Self::hash_content(content);
        self.hashes.insert(doc_id.to_string(), hash);

        // Record mtime if path provided
        if let Some(path) = path {
            if let Ok(metadata) = std::fs::metadata(path) {
                if let Ok(mtime) = metadata.modified() {
                    self.mtimes.insert(doc_id.to_string(), mtime);
                }
            }
        }
    }

    /// Compare two trees and detect changes.
    pub fn detect_changes(&self, old_tree: &VectorlessTree, new_tree: &VectorlessTree) -> ChangeSet {
        let mut changes = ChangeSet::new();

        // Collect nodes from both trees
        let old_nodes = self.collect_node_info(old_tree);
        let new_nodes = self.collect_node_info(new_tree);

        // Find added nodes
        for (title, info) in &new_nodes {
            if !old_nodes.contains_key(title) {
                changes.added.push(NodeChange {
                    node_id: info.node_id.clone(),
                    title: title.clone(),
                    change_type: ChangeType::Added,
                });
            }
        }

        // Find removed nodes
        for (title, info) in &old_nodes {
            if !new_nodes.contains_key(title) {
                changes.removed.push(NodeChange {
                    node_id: info.node_id.clone(),
                    title: title.clone(),
                    change_type: ChangeType::Removed,
                });
            }
        }

        // Find modified nodes
        for (title, new_info) in &new_nodes {
            if let Some(old_info) = old_nodes.get(title) {
                if old_info.content_hash != new_info.content_hash {
                    changes.modified.push(NodeChange {
                        node_id: new_info.node_id.clone(),
                        title: title.clone(),
                        change_type: ChangeType::Modified,
                    });
                }
            }
        }

        changes
    }

    /// Collect node information from a tree.
    fn collect_node_info(&self, tree: &VectorlessTree) -> HashMap<String, NodeInfo> {
        let mut info = HashMap::new();

        for node_id in tree.traverse() {
            if let Some(node) = tree.get(node_id) {
                // Skip root node
                if node.depth == 0 {
                    continue;
                }

                info.insert(node.title.clone(), NodeInfo {
                    node_id: node.node_id.clone(),
                    content_hash: Self::hash_content(&node.content),
                    child_count: tree.children(node_id).len(),
                });
            }
        }

        info
    }

    /// Clear stored data for a document.
    pub fn clear(&mut self, doc_id: &str) {
        self.hashes.remove(doc_id);
        self.mtimes.remove(doc_id);
    }
}

impl Default for ChangeDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal node information for change detection.
struct NodeInfo {
    /// Node ID (if assigned).
    node_id: Option<String>,
    /// Hash of node content.
    content_hash: u64,
    /// Number of direct children.
    child_count: usize,
}
