// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Change detection for incremental updates.
//!
//! This module provides fine-grained change detection using subtree fingerprints,
//! enabling precise identification of changed nodes without full reprocessing.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use vectorless_document::{DocumentTree, NodeId};
use vectorless_utils::fingerprint::{Fingerprint, Fingerprinter, NodeFingerprint};

/// Type of change detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl std::fmt::Display for ChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeType::Added => write!(f, "added"),
            ChangeType::Removed => write!(f, "removed"),
            ChangeType::Modified => write!(f, "modified"),
            ChangeType::Restructured => write!(f, "restructured"),
        }
    }
}

/// A single change in the document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeChange {
    /// Node ID (from old tree).
    pub node_id: Option<String>,
    /// Node title (for human-readable output).
    pub title: String,
    /// Type of change.
    pub change_type: ChangeType,
    /// Node fingerprint (for modified nodes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<NodeFingerprint>,
}

impl NodeChange {
    /// Create a new node change.
    pub fn new(node_id: Option<String>, title: String, change_type: ChangeType) -> Self {
        Self {
            node_id,
            title,
            change_type,
            fingerprint: None,
        }
    }

    /// Add fingerprint information.
    pub fn with_fingerprint(mut self, fp: NodeFingerprint) -> Self {
        self.fingerprint = Some(fp);
        self
    }
}

/// Set of changes between two document versions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChangeSet {
    /// Added nodes.
    pub added: Vec<NodeChange>,
    /// Removed nodes.
    pub removed: Vec<NodeChange>,
    /// Modified nodes (content changed).
    pub modified: Vec<NodeChange>,
    /// Restructured nodes (children changed).
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
        self.added.len() + self.removed.len() + self.modified.len() + self.restructured.len()
    }

    /// Merge another change set into this one.
    pub fn merge(&mut self, other: ChangeSet) {
        self.added.extend(other.added);
        self.removed.extend(other.removed);
        self.modified.extend(other.modified);
        self.restructured.extend(other.restructured);
    }

    /// Get all changed node IDs.
    pub fn changed_node_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = Vec::new();
        for change in &self.added {
            if let Some(ref id) = change.node_id {
                ids.push(id.as_str());
            }
        }
        for change in &self.modified {
            if let Some(ref id) = change.node_id {
                ids.push(id.as_str());
            }
        }
        for change in &self.restructured {
            if let Some(ref id) = change.node_id {
                ids.push(id.as_str());
            }
        }
        ids
    }
}

/// Document-level change detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChangeInfo {
    /// Document ID.
    pub doc_id: String,
    /// Overall content fingerprint.
    pub content_fp: Fingerprint,
    /// Node-level fingerprints.
    pub node_fingerprints: HashMap<String, NodeFingerprint>,
    /// Last modification time.
    pub modified_at: chrono::DateTime<chrono::Utc>,
    /// Processing version (incremented when processing algorithm changes).
    pub processing_version: u32,
}

impl DocumentChangeInfo {
    /// Create a new document change info.
    pub fn new(doc_id: &str) -> Self {
        Self {
            doc_id: doc_id.to_string(),
            content_fp: Fingerprint::zero(),
            node_fingerprints: HashMap::new(),
            modified_at: chrono::Utc::now(),
            processing_version: 1,
        }
    }

    /// Update from a tree.
    pub fn update_from_tree(&mut self, tree: &DocumentTree) {
        self.content_fp = compute_tree_fingerprint(tree);
        self.node_fingerprints = compute_all_node_fingerprints(tree);
        self.modified_at = chrono::Utc::now();
    }
}

/// Change detector for incremental updates.
///
/// Supports both simple hash-based detection and fine-grained
/// subtree fingerprint-based detection.
pub struct ChangeDetector {
    /// Content fingerprints by document ID.
    content_fps: HashMap<String, Fingerprint>,

    /// Node-level fingerprints by document ID.
    node_fps: HashMap<String, HashMap<String, NodeFingerprint>>,

    /// File modification times by document ID.
    mtimes: HashMap<String, SystemTime>,

    /// Processing versions by document ID.
    processing_versions: HashMap<String, u32>,

    /// Current processing version (for algorithm upgrades).
    current_processing_version: u32,
}

impl ChangeDetector {
    /// Create a new change detector.
    pub fn new() -> Self {
        Self {
            content_fps: HashMap::new(),
            node_fps: HashMap::new(),
            mtimes: HashMap::new(),
            processing_versions: HashMap::new(),
            current_processing_version: 1,
        }
    }

    /// Set the current processing version.
    pub fn with_processing_version(mut self, version: u32) -> Self {
        self.current_processing_version = version;
        self
    }

    /// Compute hash of content (simple u64 hash).
    fn hash_content(content: &str) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Check if a file needs recompilation based on mtime.
    pub fn needs_recompile_by_mtime(&self, doc_id: &str, path: &Path) -> bool {
        let Some(recorded_mtime) = self.mtimes.get(doc_id) else {
            return true; // Never compiled
        };

        let Ok(metadata) = std::fs::metadata(path) else {
            return true; // Can't read file
        };

        let Ok(current_mtime) = metadata.modified() else {
            return true;
        };

        current_mtime > *recorded_mtime
    }

    /// Check if content needs recompilation based on fingerprint.
    pub fn needs_recompile_by_hash(&self, doc_id: &str, content: &str) -> bool {
        let current_fp = Fingerprint::from_str(content);

        match self.content_fps.get(doc_id) {
            Some(recorded_fp) => recorded_fp != &current_fp,
            None => true,
        }
    }

    /// Check if document needs recompilation based on fingerprint.
    pub fn needs_recompile_by_fingerprint(&self, doc_id: &str, new_fp: &Fingerprint) -> bool {
        match self.content_fps.get(doc_id) {
            Some(recorded_fp) => recorded_fp != new_fp,
            None => true,
        }
    }

    /// Check if processing version has changed.
    pub fn needs_recompile_by_version(&self, doc_id: &str) -> bool {
        match self.processing_versions.get(doc_id) {
            Some(recorded_version) => *recorded_version < self.current_processing_version,
            None => true,
        }
    }

    /// Record document state after compiling.
    pub fn record(&mut self, doc_id: &str, content: &str, path: Option<&Path>) {
        self.record_with_tree(doc_id, content, None, path);
    }

    /// Record document state with tree (for fine-grained detection).
    pub fn record_with_tree(
        &mut self,
        doc_id: &str,
        content: &str,
        tree: Option<&DocumentTree>,
        path: Option<&Path>,
    ) {
        // Record content fingerprint
        let content_fp = Fingerprint::from_str(content);
        self.content_fps.insert(doc_id.to_string(), content_fp);

        // Record node fingerprints if tree provided
        if let Some(tree) = tree {
            let node_fps = compute_all_node_fingerprints(tree);
            self.node_fps.insert(doc_id.to_string(), node_fps);
        }

        // Record mtime if path provided
        if let Some(path) = path {
            if let Ok(metadata) = std::fs::metadata(path) {
                if let Ok(mtime) = metadata.modified() {
                    self.mtimes.insert(doc_id.to_string(), mtime);
                }
            }
        }

        // Record processing version
        self.processing_versions
            .insert(doc_id.to_string(), self.current_processing_version);
    }

    /// Record document from ChangeInfo.
    pub fn record_change_info(&mut self, info: &DocumentChangeInfo, path: Option<&Path>) {
        self.content_fps
            .insert(info.doc_id.clone(), info.content_fp);
        self.node_fps
            .insert(info.doc_id.clone(), info.node_fingerprints.clone());
        self.processing_versions
            .insert(info.doc_id.clone(), info.processing_version);

        if let Some(path) = path {
            if let Ok(metadata) = std::fs::metadata(path) {
                if let Ok(mtime) = metadata.modified() {
                    self.mtimes.insert(info.doc_id.clone(), mtime);
                }
            }
        }
    }

    /// Detect changes between two trees using fingerprints.
    pub fn detect_changes(&self, old_tree: &DocumentTree, new_tree: &DocumentTree) -> ChangeSet {
        let mut changes = ChangeSet::new();

        // Collect fingerprints from both trees
        let old_fps = compute_all_node_fingerprints(old_tree);
        let new_fps = compute_all_node_fingerprints(new_tree);

        // Build title -> (string_key, Fingerprint) maps by traversing trees
        // We store owned Strings to avoid lifetime issues
        let old_by_title: HashMap<String, (String, NodeFingerprint)> = {
            let mut map = HashMap::new();
            for node_id in old_tree.traverse() {
                if let Some(node) = old_tree.get(node_id) {
                    let key = node
                        .node_id
                        .clone()
                        .unwrap_or_else(|| format!("node_{:?}", node_id.0));
                    if let Some(fp) = old_fps.get(&key) {
                        map.insert(node.title.clone(), (key, fp.clone()));
                    }
                }
            }
            map
        };

        let new_by_title: HashMap<String, (String, NodeFingerprint)> = {
            let mut map = HashMap::new();
            for node_id in new_tree.traverse() {
                if let Some(node) = new_tree.get(node_id) {
                    let key = node
                        .node_id
                        .clone()
                        .unwrap_or_else(|| format!("node_{:?}", node_id.0));
                    if let Some(fp) = new_fps.get(&key) {
                        map.insert(node.title.clone(), (key, fp.clone()));
                    }
                }
            }
            map
        };

        // Find added nodes
        for (title, (node_key, fp)) in &new_by_title {
            if !old_by_title.contains_key(title) {
                changes.added.push(
                    NodeChange::new(Some(node_key.clone()), title.clone(), ChangeType::Added)
                        .with_fingerprint(fp.clone()),
                );
            }
        }

        // Find removed nodes
        for (title, (node_key, fp)) in &old_by_title {
            if !new_by_title.contains_key(title) {
                changes.removed.push(
                    NodeChange::new(Some(node_key.clone()), title.clone(), ChangeType::Removed)
                        .with_fingerprint(fp.clone()),
                );
            }
        }

        // Find modified nodes
        for (title, (new_key, new_fp)) in &new_by_title {
            if let Some((_old_key, old_fp)) = old_by_title.get(title) {
                if new_fp.content_changed(old_fp) {
                    changes.modified.push(
                        NodeChange::new(Some(new_key.clone()), title.clone(), ChangeType::Modified)
                            .with_fingerprint(new_fp.clone()),
                    );
                } else if new_fp.subtree_changed(old_fp) {
                    changes.restructured.push(
                        NodeChange::new(
                            Some(new_key.clone()),
                            title.clone(),
                            ChangeType::Restructured,
                        )
                        .with_fingerprint(new_fp.clone()),
                    );
                }
            }
        }

        changes
    }

    /// Get nodes that need reprocessing (summary regeneration).
    ///
    /// This returns nodes where either:
    /// - Content changed (summary may need update)
    /// - Processing version changed (all summaries need update)
    pub fn get_nodes_needing_reprocess(
        &self,
        doc_id: &str,
        new_tree: &DocumentTree,
    ) -> Option<Vec<String>> {
        let old_fps = self.node_fps.get(doc_id)?;
        let new_fps = compute_all_node_fingerprints(new_tree);

        let mut needs_reprocess = Vec::new();

        // If processing version changed, all nodes need reprocessing
        if self.needs_recompile_by_version(doc_id) {
            return Some(new_fps.keys().cloned().collect());
        }

        // Otherwise, only changed nodes need reprocessing
        for (node_key, new_fp) in &new_fps {
            if let Some(old_fp) = old_fps.get(node_key) {
                // Content changed or subtree structure changed
                if new_fp.content_changed(old_fp) || new_fp.subtree_changed(old_fp) {
                    needs_reprocess.push(node_key.clone());
                }
            } else {
                // New node
                needs_reprocess.push(node_key.clone());
            }
        }

        Some(needs_reprocess)
    }

    /// Clear stored data for a document.
    pub fn clear(&mut self, doc_id: &str) {
        self.content_fps.remove(doc_id);
        self.node_fps.remove(doc_id);
        self.mtimes.remove(doc_id);
        self.processing_versions.remove(doc_id);
    }

    /// Get the current content fingerprint for a document.
    pub fn get_content_fingerprint(&self, doc_id: &str) -> Option<&Fingerprint> {
        self.content_fps.get(doc_id)
    }

    /// Get all node fingerprints for a document.
    pub fn get_node_fingerprints(&self, doc_id: &str) -> Option<&HashMap<String, NodeFingerprint>> {
        self.node_fps.get(doc_id)
    }

    /// Serialize state for persistence.
    pub fn to_state(&self) -> ChangeDetectorState {
        ChangeDetectorState {
            content_fps: self.content_fps.clone(),
            node_fps: self.node_fps.clone(),
            processing_versions: self.processing_versions.clone(),
        }
    }

    /// Restore state from persistence.
    pub fn from_state(state: ChangeDetectorState) -> Self {
        Self {
            content_fps: state.content_fps,
            node_fps: state.node_fps,
            mtimes: HashMap::new(),
            processing_versions: state.processing_versions,
            current_processing_version: 1,
        }
    }
}

impl Default for ChangeDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializable state for change detector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDetectorState {
    /// Content fingerprints by document ID.
    pub content_fps: HashMap<String, Fingerprint>,
    /// Node fingerprints by document ID.
    pub node_fps: HashMap<String, HashMap<String, NodeFingerprint>>,
    /// Processing versions by document ID.
    pub processing_versions: HashMap<String, u32>,
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Compute the overall fingerprint for a tree.
pub fn compute_tree_fingerprint(tree: &DocumentTree) -> Fingerprint {
    let root_fp = compute_node_fingerprint(tree, tree.root());
    root_fp.subtree
}

/// Compute content fingerprint for a single node.
fn compute_node_content_fp(tree: &DocumentTree, node_id: NodeId) -> Fingerprint {
    let node = match tree.get(node_id) {
        Some(n) => n,
        None => return Fingerprint::zero(),
    };

    Fingerprinter::new()
        .with_str(&node.title)
        .with_str(&node.content)
        .with_option_str(node.node_id.as_deref())
        .into_fingerprint()
}

/// Compute fingerprint for a node and its subtree.
fn compute_node_fingerprint(tree: &DocumentTree, node_id: NodeId) -> NodeFingerprint {
    let node = match tree.get(node_id) {
        Some(n) => n,
        None => return NodeFingerprint::zero(),
    };

    // Content fingerprint
    let content_fp = Fingerprinter::new()
        .with_str(&node.title)
        .with_str(&node.content)
        .with_option_str(node.node_id.as_deref())
        .into_fingerprint();

    // Check if leaf node
    let children = tree.children(node_id);
    if children.is_empty() {
        return NodeFingerprint::leaf(content_fp);
    }

    // Compute subtree fingerprint from children
    let mut subtree_fp = Fingerprinter::new();
    subtree_fp.write_fingerprint(&content_fp);

    for child_id in children {
        let child_fp = compute_node_fingerprint(tree, child_id);
        subtree_fp.write_fingerprint(&child_fp.subtree);
    }

    NodeFingerprint::new(content_fp, subtree_fp.into_fingerprint())
}

/// Compute fingerprints for all nodes in a tree.
/// Returns a map from string key (for persistence) to NodeFingerprint.
pub fn compute_all_node_fingerprints(tree: &DocumentTree) -> HashMap<String, NodeFingerprint> {
    let mut fingerprints = HashMap::new();

    for node_id in tree.traverse() {
        if let Some(node) = tree.get(node_id) {
            let key = node
                .node_id
                .clone()
                .unwrap_or_else(|| format!("node_{:?}", node_id.0));
            let fp = compute_node_fingerprint(tree, node_id);
            fingerprints.insert(key, fp);
        }
    }

    fingerprints
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorless_document::DocumentTree;

    #[test]
    fn test_change_detector_new() {
        let detector = ChangeDetector::new();
        assert!(detector.content_fps.is_empty());
    }

    #[test]
    fn test_needs_recompile_by_hash() {
        let mut detector = ChangeDetector::new();

        // First time: always needs recompilation
        assert!(detector.needs_recompile_by_hash("doc1", "content"));

        // Record the content
        detector.record("doc1", "content", None);

        // Same content: no recompilation needed
        assert!(!detector.needs_recompile_by_hash("doc1", "content"));

        // Different content: needs recompilation
        assert!(detector.needs_recompile_by_hash("doc1", "new content"));
    }

    #[test]
    fn test_change_set() {
        let mut changes = ChangeSet::new();
        assert!(changes.is_empty());

        changes.added.push(NodeChange::new(
            Some("node1".to_string()),
            "Title".to_string(),
            ChangeType::Added,
        ));

        assert!(!changes.is_empty());
        assert_eq!(changes.total_changes(), 1);
    }

    #[test]
    fn test_processing_version() {
        let mut detector = ChangeDetector::new().with_processing_version(2);
        detector.record("doc1", "content", None);

        // Version matches, no recompilation needed
        assert!(!detector.needs_recompile_by_version("doc1"));

        // Create new detector with higher version
        let detector2 = ChangeDetector::new().with_processing_version(3);
        assert!(detector2.needs_recompile_by_version("doc1"));
    }

    #[test]
    fn test_node_fingerprint() {
        let mut tree = DocumentTree::new("Root", "root content");
        let child = tree.add_child(tree.root(), "Child", "child content");

        let root_fp = compute_node_fingerprint(&tree, tree.root());
        let child_fp = compute_node_fingerprint(&tree, child);

        // Child is a leaf, content == subtree
        assert_eq!(child_fp.content, child_fp.subtree);

        // Root is not a leaf
        assert_ne!(root_fp.content, root_fp.subtree);
    }

    #[test]
    fn test_fingerprint_serialization() {
        let mut detector = ChangeDetector::new();
        let mut tree = DocumentTree::new("Root", "content");
        tree.add_child(tree.root(), "Section", "section content");

        detector.record_with_tree("doc1", "content", Some(&tree), None);

        let state = detector.to_state();
        let json = serde_json::to_string(&state).unwrap();
        let restored: ChangeDetectorState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.content_fps, restored.content_fps);
    }
}
