// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document navigation — the core type for navigating an understood document.
//!
//! `DocumentNavigator` holds an owned `Document` plus mutable navigation state
//! (cursor, breadcrumb, visited set, collected evidence). All methods are
//! `async` so they integrate naturally with the PyO3 async bridge.
//!
//! Implementation is split across files:
//! - `navigator.rs` (this file) — struct definition, core navigation, content, evidence
//! - `navigator_inspection.rs` — P1 inspection tools (toc, stats, siblings, etc.)
//! - `navigator_search.rs` — search tools (grep, find, similar, etc.)

use std::collections::{HashMap, HashSet};

use vectorless_document::{Document, NodeId};
use vectorless_error::{Error, Result};

use crate::resolve::resolve_target_extended;
use crate::subtree::collect_subtree;
use crate::types::*;

/// Navigation state machine over a single understood document.
///
/// Created from a [`Document`] (produced by the compile pipeline).
/// Python Worker holds one `DocumentNavigator` and calls navigation methods
/// to traverse the document tree, collect evidence, and query indexes.
pub struct DocumentNavigator {
    doc: Document,
    cursor: NodeId,
    breadcrumb: Vec<String>,
    /// Navigation history stack for `back()`. Pushed on every cd/cd_by_title.
    history: Vec<NodeId>,
    node_id_map: HashMap<u64, NodeId>,
    visited: HashSet<NodeId>,
    collected: HashSet<NodeId>,
    evidence: Vec<CollectedEvidence>,
}

impl DocumentNavigator {
    /// Create a new navigator starting at the document root.
    pub fn new(doc: Document) -> Self {
        let cursor = doc.tree.root();
        let mut node_id_map = HashMap::new();
        for id in doc.tree.traverse() {
            node_id_map.insert(usize::from(id.0) as u64, id);
        }
        Self {
            doc,
            cursor,
            breadcrumb: vec!["root".to_string()],
            history: Vec::new(),
            node_id_map,
            visited: HashSet::new(),
            collected: HashSet::new(),
            evidence: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // NodeId bridge
    // -----------------------------------------------------------------------

    fn parse_id(&self, s: &str) -> Result<NodeId> {
        let num: u64 = s
            .strip_prefix('n')
            .ok_or_else(|| Error::InvalidInput(format!("NodeId must start with 'n', got: {s}")))?
            .parse()
            .map_err(|_| Error::InvalidInput(format!("Invalid NodeId: {s}")))?;
        self.node_id_map
            .get(&num)
            .copied()
            .ok_or_else(|| Error::NodeNotFound(format!("n{num}")))
    }

    fn id_to_u64(&self, id: NodeId) -> u64 {
        usize::from(id.0) as u64
    }

    fn resolve_optional_id(&self, opt: Option<&str>) -> Result<NodeId> {
        match opt {
            Some(s) => self.parse_id(s),
            None => Ok(self.cursor),
        }
    }

    // -----------------------------------------------------------------------
    // Navigation
    // -----------------------------------------------------------------------

    /// List children of the current node with rich metadata.
    pub async fn ls(&self) -> Vec<NodeInfo> {
        let routes = self.doc.nav_index.get_child_routes(self.cursor);
        match routes {
            Some(routes) => routes
                .iter()
                .map(|route| {
                    let child_count = self.doc.tree.children(route.node_id).len();
                    let (hints, tags, leaf_count) = self
                        .doc
                        .nav_index
                        .get_entry(route.node_id)
                        .map(|e| (e.question_hints.clone(), e.topic_tags.clone(), e.leaf_count))
                        .unwrap_or_default();
                    let depth = self.doc.tree.depth(route.node_id);
                    NodeInfo {
                        id: self.id_to_u64(route.node_id),
                        title: route.title.clone(),
                        depth,
                        child_count,
                        leaf_count,
                        question_hints: hints,
                        topic_tags: tags,
                    }
                })
                .collect(),
            None => Vec::new(),
        }
    }

    /// Navigate to a specific node by numeric id.
    pub async fn cd(&mut self, node_id: &str) -> Result<()> {
        let id = self.parse_id(node_id)?;
        self.visited.insert(id);
        let title = self
            .doc
            .tree
            .get(id)
            .map(|n| n.title.as_str())
            .unwrap_or("unknown")
            .to_string();
        self.history.push(self.cursor);
        self.cursor = id;
        self.breadcrumb.push(title);
        Ok(())
    }

    /// Navigate to a child by title (fuzzy matching via resolve_target_extended).
    pub async fn cd_by_title(&mut self, title: &str) -> Result<()> {
        let id = resolve_target_extended(title, &self.doc.nav_index, self.cursor, &self.doc.tree)
            .ok_or_else(|| {
            Error::NodeNotFound(format!(
                "Target '{title}' not found. Use ls to see children."
            ))
        })?;
        let resolved_title = self
            .doc
            .tree
            .get(id)
            .map(|n| n.title.as_str())
            .unwrap_or(title)
            .to_string();
        self.visited.insert(id);
        self.history.push(self.cursor);
        self.cursor = id;
        self.breadcrumb.push(resolved_title);
        Ok(())
    }

    /// Navigate up to the parent of the current node.
    pub async fn cd_up(&mut self) -> Result<()> {
        if self.breadcrumb.len() <= 1 {
            return Err(Error::InvalidInput("Already at root.".into()));
        }
        let parent = self
            .doc
            .tree
            .parent(self.cursor)
            .ok_or_else(|| Error::NodeNotFound("No parent.".into()))?;
        self.breadcrumb.pop();
        self.cursor = parent;
        Ok(())
    }

    /// Navigate back to the root node.
    pub async fn cd_root(&mut self) {
        self.cursor = self.doc.tree.root();
        self.breadcrumb = vec!["root".to_string()];
    }

    /// Go back to the previous position (uses navigation history stack).
    pub async fn back(&mut self) -> Result<()> {
        let prev = self
            .history
            .pop()
            .ok_or_else(|| Error::InvalidInput("No previous position.".into()))?;
        self.cursor = prev;
        self._rebuild_breadcrumb();
        Ok(())
    }

    /// Return the current navigation path (e.g., "root / Chapter 1 / Section 1.2").
    pub async fn pwd(&self) -> String {
        self.breadcrumb.join(" / ")
    }

    // -----------------------------------------------------------------------
    // Content
    // -----------------------------------------------------------------------

    /// Read a node's content and collect it as evidence.
    /// `node_id` is `"n42"` or None for current node.
    pub async fn cat(&mut self, node_id: Option<&str>) -> Result<String> {
        let id = self.resolve_optional_id(node_id)?;
        let node = self
            .doc
            .tree
            .get(id)
            .ok_or_else(|| Error::NodeNotFound("Node not found.".into()))?;

        let title = node.title.clone();
        let content = node.content.clone();

        if !content.is_empty() && !self.collected.contains(&id) {
            let source_path = self.breadcrumb.join(" / ") + " / " + &title;
            self.evidence.push(CollectedEvidence {
                node_id: self.id_to_u64(id),
                title: title.clone(),
                content: content.clone(),
                source_path,
            });
            self.collected.insert(id);
        }
        self.visited.insert(id);

        Ok(content)
    }

    /// Preview the first N lines of a node without collecting evidence.
    pub async fn head(&self, node_id: Option<&str>, n: usize) -> Result<String> {
        let id = self.resolve_optional_id(node_id)?;
        let node = self
            .doc
            .tree
            .get(id)
            .ok_or_else(|| Error::NodeNotFound("Node not found.".into()))?;

        let content = &node.content;
        let title = &node.title;
        let total_lines = content.lines().count();
        let preview: Vec<&str> = content.lines().take(n).collect();

        let mut output = format!(
            "[Preview: {title} — showing {}/{total_lines} lines]\n",
            preview.len().min(n)
        );
        output.push_str(&preview.join("\n"));

        if total_lines > n {
            output.push_str(&format!(
                "\n... ({} more lines, use cat to read all)",
                total_lines - n
            ));
        }

        Ok(output)
    }

    /// Count lines, words, and characters in a node's content.
    pub async fn wc(&self, node_id: Option<&str>) -> Result<WordCount> {
        let id = self.resolve_optional_id(node_id)?;
        let content = self
            .doc
            .tree
            .get(id)
            .map(|n| n.content.as_str())
            .ok_or_else(|| Error::NodeNotFound("Node not found.".into()))?;

        Ok(WordCount {
            lines: content.lines().count(),
            words: content.split_whitespace().count(),
            chars: content.len(),
        })
    }

    // -----------------------------------------------------------------------
    // Metadata
    // -----------------------------------------------------------------------

    /// Document-level summary.
    pub async fn summary(&self) -> &str {
        &self.doc.summary
    }

    /// Number of sections in the tree.
    pub async fn section_count(&self) -> usize {
        self.doc.section_count
    }

    /// Document ID.
    pub async fn doc_id(&self) -> &str {
        &self.doc.doc_id
    }

    /// Document name.
    pub async fn doc_name(&self) -> &str {
        &self.doc.name
    }

    // -----------------------------------------------------------------------
    // Evidence
    // -----------------------------------------------------------------------

    /// Explicitly collect evidence from a node.
    pub async fn collect_evidence(&mut self, node_id: &str) -> Result<()> {
        let id = self.parse_id(node_id)?;
        if self.collected.contains(&id) {
            return Ok(());
        }
        let node = self
            .doc
            .tree
            .get(id)
            .ok_or_else(|| Error::NodeNotFound("Node not found.".into()))?;

        let title = node.title.clone();
        let content = node.content.clone();
        if !content.is_empty() {
            let source_path = self.breadcrumb.join(" / ") + " / " + &title;
            self.evidence.push(CollectedEvidence {
                node_id: self.id_to_u64(id),
                title,
                content,
                source_path,
            });
        }
        self.collected.insert(id);
        self.visited.insert(id);
        Ok(())
    }

    /// Return all collected evidence.
    pub async fn evidence(&self) -> &[CollectedEvidence] {
        &self.evidence
    }

    /// Clear all collected evidence.
    pub async fn clear_evidence(&mut self) {
        self.evidence.clear();
        self.collected.clear();
    }

    // -----------------------------------------------------------------------
    // Tree inspection
    // -----------------------------------------------------------------------

    /// Root node id.
    pub async fn root_id(&self) -> u64 {
        self.id_to_u64(self.doc.tree.root())
    }

    /// Current cursor node id.
    pub async fn current_id(&self) -> u64 {
        self.id_to_u64(self.cursor)
    }

    /// List children of an arbitrary node.
    pub async fn children_of(&self, node_id: &str) -> Result<Vec<NodeInfo>> {
        let id = self.parse_id(node_id)?;
        let routes = self.doc.nav_index.get_child_routes(id);
        match routes {
            Some(routes) => Ok(routes
                .iter()
                .map(|route| {
                    let child_count = self.doc.tree.children(route.node_id).len();
                    let (hints, tags, leaf_count) = self
                        .doc
                        .nav_index
                        .get_entry(route.node_id)
                        .map(|e| (e.question_hints.clone(), e.topic_tags.clone(), e.leaf_count))
                        .unwrap_or_default();
                    let depth = self.doc.tree.depth(route.node_id);
                    NodeInfo {
                        id: self.id_to_u64(route.node_id),
                        title: route.title.clone(),
                        depth,
                        child_count,
                        leaf_count,
                        question_hints: hints,
                        topic_tags: tags,
                    }
                })
                .collect()),
            None => Ok(Vec::new()),
        }
    }

    /// Parent of a node.
    pub async fn parent_of(&self, node_id: &str) -> Result<Option<u64>> {
        let id = self.parse_id(node_id)?;
        Ok(self.doc.tree.parent(id).map(|p| self.id_to_u64(p)))
    }

    /// Depth of a node in the tree.
    pub async fn depth_of(&self, node_id: &str) -> Result<usize> {
        let id = self.parse_id(node_id)?;
        Ok(self.doc.tree.depth(id))
    }

    /// Title of a node.
    pub async fn node_title(&self, node_id: &str) -> Result<String> {
        let id = self.parse_id(node_id)?;
        Ok(self
            .doc
            .tree
            .get(id)
            .map(|n| n.title.clone())
            .unwrap_or_default())
    }

    /// All node ids in the tree.
    pub async fn all_node_ids(&self) -> Vec<u64> {
        self.doc
            .tree
            .traverse()
            .iter()
            .map(|&id| self.id_to_u64(id))
            .collect()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Rebuild breadcrumb from root to current cursor.
    fn _rebuild_breadcrumb(&mut self) {
        let path = self.doc.tree.path_from_root(self.cursor);
        self.breadcrumb = std::iter::once("root".to_string())
            .chain(
                path.iter()
                    .skip(1)
                    .filter_map(|&id| self.doc.tree.get(id).map(|n| n.title.clone())),
            )
            .collect();
    }
}

// ---------------------------------------------------------------------------
// Split implementation files
// ---------------------------------------------------------------------------

include!("navigator_inspection.rs");
include!("navigator_search.rs");
