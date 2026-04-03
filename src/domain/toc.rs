// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Table of Contents (ToC) view generation.
//!
//! Provides utilities for generating different views of the document tree,
//! including hierarchical ToC, flat ToC, and filtered views.

use serde::{Deserialize, Serialize};

use super::node::NodeId;
use super::tree::DocumentTree;
use super::node::TreeNode;

/// A node in the Table of Contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocNode {
    /// Node title.
    pub title: String,
    /// Node ID (if available).
    pub node_id: Option<String>,
    /// Depth in the tree.
    pub depth: usize,
    /// Page range (for PDFs).
    pub page_range: Option<(usize, usize)>,
    /// Brief summary (optional).
    pub summary: Option<String>,
    /// Children nodes.
    pub children: Vec<TocNode>,
}

impl TocNode {
    /// Create a new ToC node.
    pub fn new(title: impl Into<String>, depth: usize) -> Self {
        Self {
            title: title.into(),
            node_id: None,
            depth,
            page_range: None,
            summary: None,
            children: Vec::new(),
        }
    }

    /// Set the node ID.
    pub fn with_node_id(mut self, id: impl Into<String>) -> Self {
        self.node_id = Some(id.into());
        self
    }

    /// Set the page range.
    pub fn with_page_range(mut self, start: usize, end: usize) -> Self {
        self.page_range = Some((start, end));
        self
    }

    /// Set the summary.
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Add a child node.
    pub fn add_child(&mut self, child: TocNode) {
        self.children.push(child);
    }

    /// Count total nodes in this subtree.
    pub fn count_nodes(&self) -> usize {
        1 + self.children.iter().map(|c| c.count_nodes()).sum::<usize>()
    }

    /// Count leaf nodes in this subtree.
    pub fn count_leaves(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            self.children.iter().map(|c| c.count_leaves()).sum()
        }
    }

    /// Get maximum depth in this subtree.
    pub fn max_depth(&self) -> usize {
        if self.children.is_empty() {
            self.depth
        } else {
            self.children.iter().map(|c| c.max_depth()).max().unwrap_or(self.depth)
        }
    }
}

/// Configuration for ToC generation.
#[derive(Debug, Clone)]
pub struct TocConfig {
    /// Maximum depth to include (None = unlimited).
    pub max_depth: Option<usize>,
    /// Whether to include summaries.
    pub include_summaries: bool,
    /// Whether to include page ranges.
    pub include_pages: bool,
    /// Minimum content length to include (filter out empty nodes).
    pub min_content_length: usize,
}

impl Default for TocConfig {
    fn default() -> Self {
        Self {
            max_depth: None,
            include_summaries: true,
            include_pages: true,
            min_content_length: 0,
        }
    }
}

impl TocConfig {
    /// Create new ToC config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum depth.
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Set whether to include summaries.
    pub fn with_summaries(mut self, include: bool) -> Self {
        self.include_summaries = include;
        self
    }
}

/// ToC view generator.
pub struct TocView {
    config: TocConfig,
}

impl TocView {
    /// Create a new ToC view generator.
    pub fn new() -> Self {
        Self {
            config: TocConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: TocConfig) -> Self {
        Self { config }
    }

    /// Generate ToC from a tree.
    pub fn generate(&self, tree: &DocumentTree) -> TocNode {
        self.build_toc_node(tree, tree.root(), 0)
    }

    /// Generate ToC starting from a specific node.
    pub fn generate_from(&self, tree: &DocumentTree, start: NodeId) -> TocNode {
        let depth = tree.get(start).map_or(0, |n| n.depth);
        self.build_toc_node(tree, start, depth)
    }

    /// Build a ToC node from a tree node.
    fn build_toc_node(&self, tree: &DocumentTree, node_id: NodeId, depth: usize) -> TocNode {
        let node = match tree.get(node_id) {
            Some(n) => n,
            None => return TocNode::new("Unknown", depth),
        };

        // Check depth limit
        if let Some(max) = self.config.max_depth {
            if depth > max {
                return TocNode::new("...", depth - 1);
            }
        }

        // Check minimum content length
        if node.content.len() < self.config.min_content_length && tree.children(node_id).is_empty() {
            return TocNode::new(node.title.clone(), depth);
        }

        let mut toc_node = TocNode::new(&node.title, depth)
            .with_node_id(node.node_id.clone().unwrap_or_default());

        // Add page range
        if self.config.include_pages {
            if let (Some(start), Some(end)) = (node.start_page, node.end_page) {
                toc_node = toc_node.with_page_range(start, end);
            }
        }

        // Add summary
        if self.config.include_summaries && !node.summary.is_empty() {
            toc_node = toc_node.with_summary(&node.summary);
        }

        // Recursively add children
        for child_id in tree.children(node_id) {
            let child_toc = self.build_toc_node(tree, child_id, depth + 1);
            toc_node.add_child(child_toc);
        }

        toc_node
    }

    /// Generate a flat list of ToC entries.
    pub fn generate_flat(&self, tree: &DocumentTree) -> Vec<TocEntry> {
        let mut entries = Vec::new();
        self.collect_flat_entries(tree, tree.root(), &mut entries);
        entries
    }

    fn collect_flat_entries(&self, tree: &DocumentTree, node_id: NodeId, entries: &mut Vec<TocEntry>) {
        if let Some(node) = tree.get(node_id) {
            entries.push(TocEntry {
                title: node.title.clone(),
                node_id: node.node_id.clone(),
                depth: node.depth,
                page_range: node.start_page.zip(node.end_page),
            });

            for child_id in tree.children(node_id) {
                self.collect_flat_entries(tree, child_id, entries);
            }
        }
    }

    /// Generate a filtered ToC based on a predicate.
    pub fn generate_filtered<F>(&self, tree: &DocumentTree, filter: F) -> Vec<TocNode>
    where
        F: Fn(&TreeNode) -> bool,
    {
        let mut result = Vec::new();
        self.collect_filtered(tree, tree.root(), &filter, &mut result);
        result
    }

    fn collect_filtered<F>(&self, tree: &DocumentTree, node_id: NodeId, filter: &F, result: &mut Vec<TocNode>)
    where
        F: Fn(&TreeNode) -> bool,
    {
        if let Some(node) = tree.get(node_id) {
            if filter(node) {
                let toc_node = self.build_toc_node(tree, node_id, node.depth);
                result.push(toc_node);
            }

            for child_id in tree.children(node_id) {
                self.collect_filtered(tree, child_id, filter, result);
            }
        }
    }

    /// Format ToC as markdown.
    pub fn format_markdown(&self, toc: &TocNode) -> String {
        let mut output = String::new();
        self.write_markdown(toc, &mut output, 0);
        output
    }

    fn write_markdown(&self, toc: &TocNode, output: &mut String, level: usize) {
        let indent = "  ".repeat(level);
        let bullet = if level == 0 { "-" } else { "-" };

        output.push_str(&format!("{}{} {}\n", indent, bullet, toc.title));

        if let Some(ref summary) = toc.summary {
            output.push_str(&format!("{}  > {}\n", indent, summary));
        }

        for child in &toc.children {
            self.write_markdown(child, output, level + 1);
        }
    }

    /// Format ToC as JSON.
    pub fn format_json(&self, toc: &TocNode) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(toc)
    }
}

impl Default for TocView {
    fn default() -> Self {
        Self::new()
    }
}

/// A flat ToC entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocEntry {
    /// Node title.
    pub title: String,
    /// Node ID.
    pub node_id: Option<String>,
    /// Depth in tree.
    pub depth: usize,
    /// Page range.
    pub page_range: Option<(usize, usize)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toc_node_creation() {
        let mut root = TocNode::new("Root", 0);
        let child = TocNode::new("Child", 1)
            .with_node_id("node-1")
            .with_summary("A child node");

        root.add_child(child);

        assert_eq!(root.count_nodes(), 2);
        assert_eq!(root.count_leaves(), 1);
        assert_eq!(root.max_depth(), 1);
    }

    #[test]
    fn test_toc_config() {
        let config = TocConfig::new()
            .with_max_depth(3)
            .with_summaries(false);

        assert_eq!(config.max_depth, Some(3));
        assert!(!config.include_summaries);
    }
}
