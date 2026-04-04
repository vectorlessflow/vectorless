// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document tree using arena-based allocation.
//!
//! This structure provides better memory locality and simpler
//! lifetime management compared to `Rc<RefCell<PageNode>}`.

use std::collections::HashMap;

use indextree::Arena;
use serde::{Deserialize, Serialize};

use super::node::{NodeId, TreeNode};

/// JSON structure for exporting document tree (matches PageIndex format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureNode {
    /// Node title.
    pub title: String,
    /// Unique node identifier.
    pub node_id: String,
    /// Starting line number (1-based).
    pub start_index: usize,
    /// Ending line number (1-based).
    pub end_index: usize,
    /// Generated summary (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Child nodes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<StructureNode>,
}

/// Document structure for JSON export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStructure {
    /// Document name.
    pub doc_name: String,
    /// Tree structure.
    pub structure: Vec<StructureNode>,
}

/// Pre-computed index for efficient retrieval operations.
///
/// Built once after the document tree is fully constructed.
/// Provides O(1) access to commonly needed traversal data.
#[derive(Debug, Clone)]
pub struct RetrievalIndex {
    /// All leaf nodes in the tree.
    leaves: Vec<NodeId>,

    /// Nodes grouped by depth level.
    /// level_index[0] = root, level_index[1] = level 1 nodes, etc.
    level_index: Vec<Vec<NodeId>>,

    /// Path from root to each node (inclusive).
    path_cache: HashMap<NodeId, Vec<NodeId>>,

    /// Siblings for each node (excluding self).
    siblings_cache: HashMap<NodeId, Vec<NodeId>>,

    /// Structure string to NodeId mapping.
    /// e.g., "1.2.3" -> NodeId
    structure_index: HashMap<String, NodeId>,

    /// Page number to NodeId mapping.
    /// Maps each page to the most specific (deepest) node containing it.
    page_index: HashMap<usize, NodeId>,

    /// NodeId to page range mapping.
    node_page_range: HashMap<NodeId, (usize, usize)>,

    /// Total node count.
    node_count: usize,

    /// Maximum depth in the tree.
    max_depth: usize,
}

impl RetrievalIndex {
    /// Get all leaf nodes.
    pub fn leaves(&self) -> &[NodeId] {
        &self.leaves
    }

    /// Get nodes at a specific depth level.
    ///
    /// Returns None if the level doesn't exist.
    pub fn level(&self, depth: usize) -> Option<&[NodeId]> {
        self.level_index.get(depth).map(|v| v.as_slice())
    }

    /// Get all levels.
    pub fn levels(&self) -> &[Vec<NodeId>] {
        &self.level_index
    }

    /// Get the path from root to a node (inclusive).
    ///
    /// Returns None if the node is not in the index.
    pub fn path_to(&self, node: NodeId) -> Option<&[NodeId]> {
        self.path_cache.get(&node).map(|v| v.as_slice())
    }

    /// Get siblings of a node (excluding the node itself).
    ///
    /// Returns None if the node is not in the index or has no siblings.
    pub fn siblings(&self, node: NodeId) -> Option<&[NodeId]> {
        self.siblings_cache.get(&node).map(|v| v.as_slice())
    }

    /// Find a node by its structure index.
    ///
    /// # Example
    /// ```ignore
    /// // Find section 2.1.3
    /// let node = index.find_by_structure("2.1.3");
    /// ```
    pub fn find_by_structure(&self, structure: &str) -> Option<NodeId> {
        self.structure_index.get(structure).copied()
    }

    /// Find the most specific node containing a page number.
    ///
    /// Returns the deepest node whose page range contains the given page.
    pub fn find_by_page(&self, page: usize) -> Option<NodeId> {
        self.page_index.get(&page).copied()
    }

    /// Get the page range for a node.
    pub fn page_range(&self, node: NodeId) -> Option<(usize, usize)> {
        self.node_page_range.get(&node).copied()
    }

    /// Get all structure indices.
    pub fn structures(&self) -> &HashMap<String, NodeId> {
        &self.structure_index
    }

    /// Get the total number of nodes.
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Get the maximum depth in the tree.
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Get the number of levels.
    pub fn level_count(&self) -> usize {
        self.level_index.len()
    }
}

/// A hierarchical document tree structure.
///
/// Uses an arena-based tree representation for efficient traversal
/// and node manipulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentTree {
    /// The underlying arena storing all nodes.
    arena: Arena<TreeNode>,

    /// The root node ID.
    root_id: NodeId,

    /// Cached leaf nodes (rebuilt on demand).
    #[serde(skip)]
    leaves_cache: Option<Vec<NodeId>>,
}

impl DocumentTree {
    /// Create a new document tree with a root node.
    pub fn new(title: &str, content: &str) -> Self {
        let mut arena = Arena::new();
        let root_data = TreeNode {
            title: title.to_string(),
            structure: String::new(), // Root has no structure index
            content: content.to_string(),
            summary: String::new(),
            depth: 0,
            start_index: 1,
            end_index: 1,
            start_page: None,
            end_page: None,
            node_id: None,
            physical_index: None,
            token_count: None,
        };
        let root_id = arena.new_node(root_data);

        // Root is initially a leaf
        let leaves_cache = Some(vec![NodeId(root_id)]);

        Self {
            arena,
            root_id: NodeId(root_id),
            leaves_cache,
        }
    }

    /// Create a document tree from an existing arena and root ID.
    ///
    /// This is useful for deserialization and testing.
    pub fn from_raw(arena: Arena<TreeNode>, root_id: NodeId) -> Self {
        Self {
            arena,
            root_id,
            leaves_cache: None, // Will be rebuilt on demand
        }
    }

    /// Get the root node ID.
    pub fn root(&self) -> NodeId {
        self.root_id
    }

    /// Get a reference to the underlying arena.
    pub fn arena(&self) -> &Arena<TreeNode> {
        &self.arena
    }

    /// Get a node by its ID.
    ///
    /// Returns None if the node doesn't exist.
    pub fn get(&self, id: NodeId) -> Option<&TreeNode> {
        self.arena.get(id.0).map(|n| n.get())
    }

    /// Get a mutable reference to a node by its ID.
    ///
    /// Returns None if the node doesn't exist.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut TreeNode> {
        self.arena.get_mut(id.0).map(|n| n.get_mut())
    }

    /// Add a child node to the specified parent.
    ///
    /// Returns the ID of the newly created child node.
    /// The structure is automatically calculated based on siblings.
    pub fn add_child(&mut self, parent: NodeId, title: &str, content: &str) -> NodeId {
        let parent_depth = self.arena.get(parent.0).map(|n| n.get().depth).unwrap_or(0);
        let parent_structure = self
            .arena
            .get(parent.0)
            .map(|n| n.get().structure.clone())
            .unwrap_or_default();

        // Calculate child index (1-based)
        let child_index = parent.0.children(&self.arena).count() + 1;

        // Calculate structure: parent_structure.child_index
        let child_structure = if parent_structure.is_empty() {
            child_index.to_string()
        } else {
            format!("{}.{}", parent_structure, child_index)
        };

        let child_data = TreeNode {
            title: title.to_string(),
            structure: child_structure,
            content: content.to_string(),
            summary: String::new(),
            depth: parent_depth + 1,
            start_index: 1,
            end_index: 1,
            start_page: None,
            end_page: None,
            node_id: None,
            physical_index: None,
            token_count: None,
        };
        let child_id = self.arena.new_node(child_data);
        parent.0.append(child_id, &mut self.arena);

        // Update leaves cache
        if let Some(ref mut cache) = self.leaves_cache {
            // Remove parent from leaves (it's no longer a leaf)
            cache.retain(|&id| id != parent);
            // Add child to leaves
            cache.push(NodeId(child_id));
        }

        NodeId(child_id)
    }

    /// Add a child node with page boundaries.
    ///
    /// Returns the ID of the newly created child node.
    pub fn add_child_with_pages(
        &mut self,
        parent: NodeId,
        title: &str,
        content: &str,
        start_page: usize,
        end_page: usize,
    ) -> NodeId {
        let child_id = self.add_child(parent, title, content);
        if let Some(node) = self.get_mut(child_id) {
            node.start_page = Some(start_page);
            node.end_page = Some(end_page);
        }
        child_id
    }

    /// Check if a node is a leaf (has no children).
    pub fn is_leaf(&self, id: NodeId) -> bool {
        id.0.children(&self.arena).next().is_none()
    }

    /// Get the number of children of a node.
    ///
    /// This is more efficient than `children().len()` as it doesn't allocate.
    pub fn child_count(&self, id: NodeId) -> usize {
        id.0.children(&self.arena).count()
    }

    /// Get the children of a node as an iterator.
    ///
    /// Use this instead of `children()` when you only need to iterate,
    /// as it avoids allocating a Vec.
    pub fn children_iter(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        id.0.children(&self.arena).map(NodeId)
    }

    /// Get the children of a node.
    ///
    /// Returns a Vec for cases where you need owned access to the children.
    /// Consider using `children_iter()` if you only need to iterate.
    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        self.children_iter(id).collect()
    }

    /// Get the parent of a node.
    ///
    /// Returns None if the node is the root or doesn't have a parent.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        id.0.parent(&self.arena).map(NodeId)
    }

    /// Get the siblings of a node (excluding the node itself).
    ///
    /// Returns an empty iterator for the root node.
    pub fn siblings_iter(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        id.0.preceding_siblings(&self.arena)
            .chain(id.0.following_siblings(&self.arena))
            .map(NodeId)
    }

    /// Get the ancestors of a node from parent to root.
    ///
    /// Returns an empty iterator for the root node.
    pub fn ancestors_iter(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        id.0.ancestors(&self.arena).map(NodeId)
    }

    /// Get the path from root to a node (inclusive).
    ///
    /// Returns the path as a Vec starting from the root.
    pub fn path_from_root(&self, id: NodeId) -> Vec<NodeId> {
        let mut path: Vec<NodeId> = self.ancestors_iter(id).collect();
        path.reverse();
        path.push(id);
        path
    }

    /// Get the depth of a node (root = 0).
    pub fn depth(&self, id: NodeId) -> usize {
        self.get(id).map(|n| n.depth).unwrap_or(0)
    }

    /// Get the first child of a node.
    ///
    /// Returns None if the node has no children.
    pub fn first_child(&self, id: NodeId) -> Option<NodeId> {
        self.children_iter(id).next()
    }

    /// Get the last child of a node.
    ///
    /// Returns None if the node has no children.
    pub fn last_child(&self, id: NodeId) -> Option<NodeId> {
        self.children_iter(id).last()
    }

    /// Get all leaf nodes in the tree.
    ///
    /// Uses cached leaves if available, otherwise rebuilds the cache.
    pub fn leaves(&self) -> Vec<NodeId> {
        if let Some(ref cache) = self.leaves_cache {
            return cache.clone();
        }

        // Rebuild cache on demand
        let leaves: Vec<NodeId> = self
            .traverse()
            .into_iter()
            .filter(|id| self.is_leaf(*id))
            .collect();

        // Note: Can't mutate self here, caller should use rebuild_leaves_cache()
        leaves
    }

    /// Rebuild the leaves cache.
    ///
    /// Call this after deserialization or batch modifications.
    pub fn rebuild_leaves_cache(&mut self) {
        self.leaves_cache = Some(
            self.traverse()
                .into_iter()
                .filter(|id| self.is_leaf(*id))
                .collect(),
        );
    }

    /// Invalidate the leaves cache.
    ///
    /// Called automatically by mutation methods.
    pub fn invalidate_leaves_cache(&mut self) {
        self.leaves_cache = None;
    }

    /// Get all nodes in the tree (depth-first order).
    pub fn traverse(&self) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut stack = vec![self.root_id];

        while let Some(id) = stack.pop() {
            result.push(id);
            // Add children in reverse order for correct DFS order
            let mut children: Vec<_> = self.children(id).into_iter().collect();
            children.reverse();
            stack.extend(children);
        }

        result
    }

    /// Get the number of nodes in the tree.
    pub fn node_count(&self) -> usize {
        self.arena.count()
    }

    /// Update a node's summary.
    pub fn set_summary(&mut self, id: NodeId, summary: &str) {
        if let Some(node) = self.get_mut(id) {
            node.summary = summary.to_string();
        }
    }

    /// Update a node's content.
    pub fn set_content(&mut self, id: NodeId, content: &str) {
        if let Some(node) = self.get_mut(id) {
            node.content = content.to_string();
        }
    }

    /// Update a node's structure index.
    pub fn set_structure(&mut self, id: NodeId, structure: &str) {
        if let Some(node) = self.get_mut(id) {
            node.structure = structure.to_string();
        }
    }

    /// Set page boundaries for a node.
    pub fn set_page_boundaries(&mut self, id: NodeId, start: usize, end: usize) {
        if let Some(node) = self.get_mut(id) {
            node.start_page = Some(start);
            node.end_page = Some(end);
        }
    }

    /// Set line indices for a node.
    pub fn set_line_indices(&mut self, id: NodeId, start: usize, end: usize) {
        if let Some(node) = self.get_mut(id) {
            node.start_index = start;
            node.end_index = end;
        }
    }

    /// Get page range for a node.
    pub fn page_range(&self, id: NodeId) -> Option<(usize, usize)> {
        let node = self.get(id)?;
        match (node.start_page, node.end_page) {
            (Some(start), Some(end)) => Some((start, end)),
            _ => None,
        }
    }

    /// Check if a node contains a specific page.
    pub fn contains_page(&self, id: NodeId, page: usize) -> bool {
        if let Some((start, end)) = self.page_range(id) {
            page >= start && page <= end
        } else {
            false
        }
    }

    /// Set the node ID (identifier string).
    pub fn set_node_id(&mut self, id: NodeId, node_id: &str) {
        if let Some(node) = self.get_mut(id) {
            node.node_id = Some(node_id.to_string());
        }
    }

    /// Set the physical index marker.
    pub fn set_physical_index(&mut self, id: NodeId, index: &str) {
        if let Some(node) = self.get_mut(id) {
            node.physical_index = Some(index.to_string());
        }
    }

    /// Update token count for a node.
    pub fn set_token_count(&mut self, id: NodeId, count: usize) {
        if let Some(node) = self.get_mut(id) {
            node.token_count = Some(count);
        }
    }

    /// Export the tree structure to JSON format (PageIndex compatible).
    pub fn to_structure_json(&self, doc_name: &str) -> DocumentStructure {
        let structure = self.build_structure_nodes(self.root_id);
        DocumentStructure {
            doc_name: doc_name.to_string(),
            structure,
        }
    }

    /// Build a retrieval index for efficient operations.
    ///
    /// This should be called once after the tree is fully constructed.
    /// The index provides O(1) access to commonly needed traversal data.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let tree = /* build tree */;
    /// let index = tree.build_retrieval_index();
    ///
    /// // Fast access to leaves
    /// for leaf in index.leaves() {
    ///     // process leaf
    /// }
    ///
    /// // Fast path lookup
    /// if let Some(path) = index.path_to(node_id) {
    ///     // path[0] = root, path[-1] = node_id
    /// }
    ///
    /// // Fast structure lookup
    /// if let Some(node) = index.find_by_structure("2.1.3") {
    ///     // Found section 2.1.3
    /// }
    ///
    /// // Fast page lookup
    /// if let Some(node) = index.find_by_page(42) {
    ///     // Found node containing page 42
    /// }
    /// ```
    pub fn build_retrieval_index(&self) -> RetrievalIndex {
        let mut leaves = Vec::new();
        let mut level_index: Vec<Vec<NodeId>> = Vec::new();
        let mut path_cache: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut siblings_cache: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut structure_index: HashMap<String, NodeId> = HashMap::new();
        let mut page_index: HashMap<usize, NodeId> = HashMap::new();
        let mut node_page_range: HashMap<NodeId, (usize, usize)> = HashMap::new();
        let mut max_depth = 0;
        let node_count = self.node_count();

        // BFS to build level index
        let mut current_level = vec![self.root_id];

        // Initialize root path
        path_cache.insert(self.root_id, vec![self.root_id]);

        while !current_level.is_empty() {
            level_index.push(current_level.clone());

            let mut next_level = Vec::new();

            for &node_id in &current_level {
                let children: Vec<NodeId> = self.children(node_id);

                // Get node data
                if let Some(node) = self.get(node_id) {
                    max_depth = max_depth.max(node.depth);

                    // Build structure index
                    if !node.structure.is_empty() {
                        structure_index.insert(node.structure.clone(), node_id);
                    }

                    // Build page index and page range
                    if let (Some(start), Some(end)) = (node.start_page, node.end_page) {
                        node_page_range.insert(node_id, (start, end));

                        // Map each page to this node (will be overwritten by deeper nodes)
                        for page in start..=end {
                            page_index.insert(page, node_id);
                        }
                    }
                }

                // Check if leaf
                if children.is_empty() {
                    leaves.push(node_id);
                }

                // Build siblings cache for children
                if children.len() > 1 {
                    for (i, &child) in children.iter().enumerate() {
                        let siblings: Vec<NodeId> = children
                            .iter()
                            .enumerate()
                            .filter(|(j, _)| *j != i)
                            .map(|(_, &c)| c)
                            .collect();
                        siblings_cache.insert(child, siblings);
                    }
                }

                // Build path cache for children
                if let Some(parent_path) = path_cache.get(&node_id).cloned() {
                    for &child in &children {
                        let mut child_path = parent_path.clone();
                        child_path.push(child);
                        path_cache.insert(child, child_path);
                    }
                }

                next_level.extend(children);
            }

            current_level = next_level;
        }

        RetrievalIndex {
            leaves,
            level_index,
            path_cache,
            siblings_cache,
            structure_index,
            page_index,
            node_page_range,
            node_count,
            max_depth,
        }
    }

    /// Recursively build structure nodes starting from the given node.
    fn build_structure_nodes(&self, node_id: NodeId) -> Vec<StructureNode> {
        let children = self.children(node_id);
        children
            .into_iter()
            .enumerate()
            .map(|(idx, child_id)| self.node_to_structure(child_id, idx))
            .collect()
    }

    /// Convert a single node to StructureNode format.
    fn node_to_structure(&self, node_id: NodeId, _idx: usize) -> StructureNode {
        let node = self.get(node_id).cloned().unwrap_or_default();
        let children = self.children(node_id);

        StructureNode {
            title: node.title,
            node_id: node
                .node_id
                .clone()
                .unwrap_or_else(|| format!("{:04}", _idx)),
            start_index: node.start_index,
            end_index: node.end_index,
            summary: if node.summary.is_empty() {
                None
            } else {
                Some(node.summary)
            },
            nodes: children
                .into_iter()
                .enumerate()
                .map(|(i, c)| self.node_to_structure(c, i))
                .collect(),
        }
    }
}

impl Default for DocumentTree {
    fn default() -> Self {
        Self::new("Root", "")
    }
}
