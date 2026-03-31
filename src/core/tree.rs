/// Document tree using arena-based allocation.
///
/// This structure provides better memory locality and simpler
/// lifetime management compared to `Rc<RefCell<PageNode>`.
/// 

use crate::TreeNode;
use indextree::Arena;
use crate::core::node::NodeId;

pub struct DocumentTree {
    /// The underlying arena storing all nodes.
    arena: Arena<TreeNode>,

    /// The root node ID.
    root_id: NodeId,
}

impl DocumentTree {
    /// Create a new document tree with a root node.
    pub fn new(title: &str, content: &str) -> Self {
        let mut arena = Arena::new();
        let root_data = TreeNode {
            title: title.to_string(),
            content: content.to_string(),
            summary: String::new(),
            depth: 0,
            start_page: None,
            end_page: None,
            node_id: None,
            physical_index: None,
            token_count: None,
        };
        let root_id = arena.new_node(root_data);

        Self { arena, root_id: NodeId(root_id) }
    }

    /// Create a document tree from an existing arena and root ID.
    ///
    /// This is useful for deserialization and testing.
    pub fn from_raw(arena: Arena<TreeNode>, root_id: NodeId) -> Self {
        Self { arena, root_id }
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
    pub fn add_child(&mut self, parent: NodeId, title: &str, content: &str) -> NodeId {
        let parent_depth = self.arena.get(parent.0).map(|n| n.get().depth).unwrap_or(0);
        let child_data = TreeNode {
            title: title.to_string(),
            content: content.to_string(),
            summary: String::new(),
            depth: parent_depth + 1,
            start_page: None,
            end_page: None,
            node_id: None,
            physical_index: None,
            token_count: None,
        };
        let child_id = self.arena.new_node(child_data);
        parent.0.append(child_id, &mut self.arena);
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

    /// Get the children of a node.
    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        id.0.children(&self.arena).map(NodeId).collect()
    }

    /// Get the parent of a node.
    ///
    /// Returns None if the node is the root or doesn't have a parent.
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        id.0.parent(&self.arena).map(NodeId)
    }

    /// Get all leaf nodes in the tree.
    pub fn leaves(&self) -> Vec<NodeId> {
        self.traverse()
            .into_iter()
            .filter(|id| self.is_leaf(*id))
            .collect()
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

    /// Set page boundaries for a node.
    pub fn set_page_boundaries(&mut self, id: NodeId, start: usize, end: usize) {
        if let Some(node) = self.get_mut(id) {
            node.start_page = Some(start);
            node.end_page = Some(end);
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

}
