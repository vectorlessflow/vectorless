// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Tree builder for constructing document trees from raw nodes.

use crate::core::{DocumentTree, NodeId};
use crate::document::RawNode;

/// Builder for constructing document trees from raw nodes.
pub struct TreeBuilder {
    /// Root title for the document.
    root_title: String,

    /// Root content for the document.
    root_content: String,
}

impl TreeBuilder {
    /// Create a new tree builder.
    pub fn new() -> Self {
        Self {
            root_title: "Root".to_string(),
            root_content: String::new(),
        }
    }

    /// Set the root title.
    pub fn with_root_title(mut self, title: impl Into<String>) -> Self {
        self.root_title = title.into();
        self
    }

    /// Set the root content.
    pub fn with_root_content(mut self, content: impl Into<String>) -> Self {
        self.root_content = content.into();
        self
    }

    /// Build a document tree from raw nodes.
    ///
    /// The raw nodes are organized into a hierarchical structure based on
    /// their levels. Level 0 nodes become children of root, level 1 nodes
    /// become children of level 0 nodes, etc.
    pub fn build(&self, raw_nodes: Vec<RawNode>) -> DocumentTree {
        let mut tree = DocumentTree::new(&self.root_title, &self.root_content);

        // Stack to track parent nodes at each level
        // Index = level, Value = NodeId at that level
        let mut level_stack: Vec<Option<NodeId>> = vec![Some(tree.root())];

        for raw in raw_nodes {
            let level = raw.level;

            // Ensure stack has enough slots
            while level_stack.len() <= level {
                level_stack.push(None);
            }

            // Find the parent: closest ancestor with a lower level
            let parent_id = (0..level)
                .rev()
                .find_map(|l| level_stack.get(l).copied().flatten())
                .unwrap_or(tree.root());

            // Create the node using the tree's API
            let content = if raw.content.is_empty() { "" } else { &raw.content };
            let node_id = tree.add_child(parent_id, &raw.title, content);

            // Update page boundaries if available
            if let Some(page) = raw.page {
                tree.set_page_boundaries(node_id, page, page);
            }

            // Update token count if available
            if let Some(count) = raw.token_count && count > 0 {
                tree.set_token_count(node_id, raw.token_count.unwrap());
            }

            // Update the stack for this level
            if level < level_stack.len() {
                level_stack[level] = Some(node_id);
            }

            // Clear deeper levels (they are no longer valid parents)
            for i in (level + 1)..level_stack.len() {
                level_stack[i] = None;
            }
        }

        tree
    }

    /// Build a tree and assign node IDs.
    pub fn build_with_ids(&self, raw_nodes: Vec<RawNode>) -> DocumentTree {
        let mut tree = self.build(raw_nodes);
        self.assign_node_ids(&mut tree);
        tree
    }

    /// Assign unique node IDs to all nodes in the tree.
    fn assign_node_ids(&self, tree: &mut DocumentTree) {
        let mut counter: usize = 0;
        self.assign_recursive(tree, tree.root(), &mut counter);
    }

    fn assign_recursive(&self, tree: &mut DocumentTree, node_id: NodeId, counter: &mut usize) {
        *counter += 1;
        let id_str = format!("node-{:04}", counter);
        tree.set_node_id(node_id, &id_str);

        // Process children
        let children = tree.children(node_id);
        for child_id in children {
            self.assign_recursive(tree, child_id, counter);
        }
    }
}

impl Default for TreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_simple_tree() {
        let raw_nodes = vec![
            RawNode { level: 1, title: "Section 1".into(), content: "Content 1".into(), ..Default::default() },
            RawNode { level: 2, title: "Subsection 1.1".into(), content: "Sub content".into(), ..Default::default() },
            RawNode { level: 1, title: "Section 2".into(), content: "Content 2".into(), ..Default::default() },
        ];

        let tree = TreeBuilder::new()
            .with_root_title("Document")
            .build(raw_nodes);

        // Root has 2 children (Section 1 and Section 2)
        let root_children = tree.children(tree.root());
        assert_eq!(root_children.len(), 2);

        // Section 1 has 1 child (Subsection 1.1)
        let section1_children = tree.children(root_children[0]);
        assert_eq!(section1_children.len(), 1);
    }
}
