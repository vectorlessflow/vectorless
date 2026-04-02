// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Partial tree updater for incremental indexing.

use tracing::info;

use crate::core::{NodeId, Result, VectorlessTree};
use crate::parser::RawNode;

use super::detector::ChangeDetector;

/// Result of a partial update.
#[derive(Debug)]
pub struct UpdateResult {
    /// Number of nodes added.
    pub nodes_added: usize,
    /// Number of nodes removed.
    pub nodes_removed: usize,
    /// Number of nodes modified.
    pub nodes_modified: usize,
    /// Number of summaries regenerated.
    pub summaries_regenerated: usize,
}

impl Default for UpdateResult {
    fn default() -> Self {
        Self {
            nodes_added: 0,
            nodes_removed: 0,
            nodes_modified: 0,
            summaries_regenerated: 0,
        }
    }
}

/// Partial updater for incremental document updates.
pub struct PartialUpdater {
    /// Change detector.
    detector: ChangeDetector,
}

impl PartialUpdater {
    /// Create a new partial updater.
    pub fn new() -> Self {
        Self {
            detector: ChangeDetector::new(),
        }
    }

    /// Get the change detector.
    pub fn detector(&self) -> &ChangeDetector {
        &self.detector
    }

    /// Get mutable change detector.
    pub fn detector_mut(&mut self) -> &mut ChangeDetector {
        &mut self.detector
    }

    /// Update a tree with new raw nodes.
    ///
    /// This performs a partial update by:
    /// 1. Detecting changes between old and new content
    /// 2. Updating only the affected subtrees
    /// 3. Regenerating summaries for changed nodes
    pub fn update(
        &self,
        old_tree: &VectorlessTree,
        new_raw_nodes: Vec<RawNode>,
    ) -> Result<(VectorlessTree, UpdateResult)> {
        let mut result = UpdateResult::default();

        // Build new tree from raw nodes
        let new_tree = self.build_tree_from_raw(new_raw_nodes)?;

        // Detect changes
        let changes = self.detector.detect_changes(old_tree, &new_tree);

        info!(
            "Detected changes: {} added, {} removed, {} modified",
            changes.added.len(),
            changes.removed.len(),
            changes.modified.len()
        );

        result.nodes_added = changes.added.len();
        result.nodes_removed = changes.removed.len();
        result.nodes_modified = changes.modified.len();

        // For now, return the new tree
        // In a full implementation, we would:
        // 1. Preserve unchanged summaries
        // 2. Only regenerate summaries for changed nodes
        // 3. Merge preserved and new content

        Ok((new_tree, result))
    }

    /// Build a tree from raw nodes (simple implementation).
    fn build_tree_from_raw(&self, raw_nodes: Vec<RawNode>) -> Result<VectorlessTree> {
        // This is a simplified implementation
        // In production, use the BuildStage

        let mut tree = VectorlessTree::new("Document", "");

        // Stack to track parent nodes at each level
        let mut level_stack: Vec<Option<NodeId>> = vec![Some(tree.root())];

        for raw in raw_nodes {
            let level = raw.level;

            // Ensure stack has enough slots
            while level_stack.len() <= level {
                level_stack.push(None);
            }

            // Find parent
            let parent_id = (0..level)
                .rev()
                .find_map(|l| level_stack.get(l).copied().flatten())
                .unwrap_or(tree.root());

            // Create node
            let content = if raw.content.is_empty() { "" } else { &raw.content };
            let node_id = tree.add_child(parent_id, &raw.title, content);

            // Set line indices
            tree.set_line_indices(node_id, raw.line_start, raw.line_end);

            // Set page if available
            if let Some(page) = raw.page {
                tree.set_page_boundaries(node_id, page, page);
            }

            // Set token count if available
            if let Some(count) = raw.token_count {
                if count > 0 {
                    tree.set_token_count(node_id, count);
                }
            }

            // Update stack
            if level < level_stack.len() {
                level_stack[level] = Some(node_id);
            }

            // Clear deeper levels
            for i in (level + 1)..level_stack.len() {
                level_stack[i] = None;
            }
        }

        Ok(tree)
    }

    /// Check if reindexing is needed.
    pub fn needs_reindex(&self, doc_id: &str, content: &str) -> bool {
        self.detector.needs_reindex_by_hash(doc_id, content)
    }

    /// Record document state after indexing.
    pub fn record(&mut self, doc_id: &str, content: &str) {
        self.detector.record(doc_id, content, None);
    }
}

impl Default for PartialUpdater {
    fn default() -> Self {
        Self::new()
    }
}
