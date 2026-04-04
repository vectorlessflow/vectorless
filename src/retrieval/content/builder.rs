// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Structure builder for aggregated content.
//!
//! This module transforms selected content into structured output formats.

use serde::{Deserialize, Serialize};

use crate::domain::DocumentTree;

use super::budget::SelectedContent;
use super::config::OutputFormatConfig;

/// Output format for structured content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Markdown format with headers.
    #[default]
    Markdown,
    /// JSON format.
    Json,
    /// Tree format.
    Tree,
    /// Flat text format.
    Flat,
}

impl From<OutputFormatConfig> for OutputFormat {
    fn from(config: OutputFormatConfig) -> Self {
        match config {
            OutputFormatConfig::Markdown => Self::Markdown,
            OutputFormatConfig::Json => Self::Json,
            OutputFormatConfig::Tree => Self::Tree,
            OutputFormatConfig::Flat => Self::Flat,
        }
    }
}

/// Tree node in the content structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentTreeNode {
    /// Node title.
    pub title: String,
    /// Node content (if any).
    pub content: Option<String>,
    /// Relevance score.
    pub score: f32,
    /// Child nodes.
    pub children: Vec<ContentTreeNode>,
}

impl ContentTreeNode {
    /// Create a new tree node.
    #[must_use]
    pub fn new(title: String) -> Self {
        Self {
            title,
            content: None,
            score: 0.0,
            children: Vec::new(),
        }
    }

    /// Add content to this node.
    #[must_use]
    pub fn with_content(mut self, content: String, score: f32) -> Self {
        self.content = Some(content);
        self.score = score;
        self
    }

    /// Add a child node.
    pub fn add_child(&mut self, child: ContentTreeNode) {
        self.children.push(child);
    }
}

/// Content tree structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentTree {
    /// Root node.
    pub root: ContentTreeNode,
    /// Total nodes in tree.
    pub total_nodes: usize,
}

impl ContentTree {
    /// Create a new content tree.
    #[must_use]
    pub fn new(root: ContentTreeNode) -> Self {
        Self {
            total_nodes: 1,
            root,
        }
    }
}

/// Metadata about aggregated content.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentMetadata {
    /// Total tokens in content.
    pub total_tokens: usize,
    /// Number of nodes included.
    pub node_count: usize,
    /// Average relevance score.
    pub avg_score: f32,
    /// Maximum depth included.
    pub max_depth: usize,
}

/// Structured content result.
#[derive(Debug, Clone)]
pub struct StructuredContent {
    /// Formatted content string.
    pub content: String,
    /// Optional tree structure.
    pub structure: Option<ContentTree>,
    /// Content metadata.
    pub metadata: ContentMetadata,
}

impl StructuredContent {
    /// Check if content is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Get content length in characters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.content.len()
    }
}

/// Builder for creating structured content output.
#[derive(Debug)]
pub struct StructureBuilder {
    /// Output format.
    format: OutputFormat,
    /// Include metadata in output.
    include_metadata: bool,
    /// Include scores in output.
    include_scores: bool,
}

impl StructureBuilder {
    /// Create a new structure builder.
    #[must_use]
    pub fn new(format: OutputFormat) -> Self {
        Self {
            format,
            include_metadata: false,
            include_scores: false,
        }
    }

    /// Create builder from config.
    #[must_use]
    pub fn from_config(format: OutputFormatConfig, include_scores: bool) -> Self {
        Self {
            format: OutputFormat::from(format),
            include_metadata: false,
            include_scores,
        }
    }

    /// Enable metadata in output.
    #[must_use]
    pub fn with_metadata(mut self) -> Self {
        self.include_metadata = true;
        self
    }

    /// Enable scores in output.
    #[must_use]
    pub fn with_scores(mut self) -> Self {
        self.include_scores = true;
        self
    }

    /// Build structured content from selected items.
    #[must_use]
    pub fn build(
        &self,
        selected: Vec<SelectedContent>,
        tree: &DocumentTree,
    ) -> StructuredContent {
        if selected.is_empty() {
            return StructuredContent {
                content: String::new(),
                structure: None,
                metadata: ContentMetadata::default(),
            };
        }

        // Calculate metadata
        let total_tokens: usize = selected.iter().map(|s| s.tokens).sum();
        let avg_score = selected.iter().map(|s| s.score).sum::<f32>() / selected.len() as f32;
        let max_depth = selected.iter().map(|s| s.depth).max().unwrap_or(0);

        let metadata = ContentMetadata {
            total_tokens,
            node_count: selected.len(),
            avg_score,
            max_depth,
        };

        // Build based on format
        let (content, structure) = match &self.format {
            OutputFormat::Markdown => self.build_markdown(selected, tree),
            OutputFormat::Json => self.build_json(selected, tree),
            OutputFormat::Tree => self.build_tree_format(selected, tree),
            OutputFormat::Flat => self.build_flat(selected),
        };

        StructuredContent {
            content,
            structure,
            metadata,
        }
    }

    /// Build Markdown format output.
    fn build_markdown(
        &self,
        selected: Vec<SelectedContent>,
        _tree: &DocumentTree,
    ) -> (String, Option<ContentTree>) {
        let mut sections = Vec::new();
        let mut current_depth = 0;

        // Sort by depth to maintain hierarchy
        let mut sorted = selected;
        sorted.sort_by(|a, b| a.depth.cmp(&b.depth));

        for content in sorted {
            // Adjust heading level based on depth
            let heading_level = (content.depth + 1).min(6);
            let heading = "#".repeat(heading_level);

            let mut section = format!("{} {}", heading, content.title);

            if self.include_scores {
                section.push_str(&format!(" *(score: {:.2})*", content.score));
            }

            section.push_str("\n\n");
            section.push_str(&content.content);

            if content.is_truncated() {
                section.push_str("\n\n*[content truncated]*");
            }

            sections.push(section);
            current_depth = current_depth.max(content.depth);
        }

        (sections.join("\n\n---\n\n"), None)
    }

    /// Build JSON format output.
    fn build_json(
        &self,
        selected: Vec<SelectedContent>,
        _tree: &DocumentTree,
    ) -> (String, Option<ContentTree>) {
        #[derive(Serialize)]
        struct JsonOutput<'a> {
            sections: Vec<JsonSection<'a>>,
        }

        #[derive(Serialize)]
        struct JsonSection<'a> {
            title: &'a str,
            content: &'a str,
            score: f32,
            depth: usize,
            truncated: bool,
        }

        let sections: Vec<_> = selected
            .iter()
            .map(|s| JsonSection {
                title: &s.title,
                content: &s.content,
                score: s.score,
                depth: s.depth,
                truncated: s.is_truncated(),
            })
            .collect();

        let output = JsonOutput { sections };
        let content = serde_json::to_string_pretty(&output).unwrap_or_default();

        (content, None)
    }

    /// Build tree format output.
    fn build_tree_format(
        &self,
        selected: Vec<SelectedContent>,
        tree: &DocumentTree,
    ) -> (String, Option<ContentTree>) {
        // Build tree structure
        let mut root = ContentTreeNode::new("Content".to_string());
        let mut node_count = 0;

        // Group by parent
        use std::collections::HashMap;
        let mut by_parent: HashMap<Option<crate::domain::NodeId>, Vec<&SelectedContent>> =
            HashMap::new();

        for content in &selected {
            let parent = tree.get(content.node_id).and_then(|_| {
                // Find parent in selected
                selected
                    .iter()
                    .find(|s| s.depth < content.depth)
                    .map(|s| Some(s.node_id))
                    .unwrap_or(None)
            });
            by_parent.entry(parent).or_default().push(content);
        }

        // Build tree recursively
        fn build_node(
            content: &SelectedContent,
            all_by_parent: &HashMap<Option<crate::domain::NodeId>, Vec<&SelectedContent>>,
        ) -> ContentTreeNode {
            let mut node = ContentTreeNode::new(content.title.clone())
                .with_content(content.content.clone(), content.score);

            if let Some(children) = all_by_parent.get(&Some(content.node_id)) {
                for child in children {
                    node.add_child(build_node(child, all_by_parent));
                }
            }

            node
        }

        // Add top-level items
        if let Some(top_level) = by_parent.get(&None) {
            for content in top_level {
                let node = build_node(content, &by_parent);
                node_count += count_nodes(&node);
                root.add_child(node);
            }
        }

        // Build string representation
        let content = render_tree(&root, 0);

        let tree_structure = ContentTree {
            root,
            total_nodes: node_count,
        };

        (content, Some(tree_structure))
    }

    /// Build flat format output.
    fn build_flat(&self, selected: Vec<SelectedContent>) -> (String, Option<ContentTree>) {
        let parts: Vec<_> = selected
            .iter()
            .map(|c| {
                let mut part = format!("[{}] {}", c.title, c.content);
                if self.include_scores {
                    part = format!("[{}] (score: {:.2}) {}", c.title, c.score, c.content);
                }
                part
            })
            .collect();

        (parts.join("\n\n"), None)
    }
}

impl Default for StructureBuilder {
    fn default() -> Self {
        Self::new(OutputFormat::default())
    }
}

/// Count nodes in a tree.
fn count_nodes(node: &ContentTreeNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

/// Render tree as string.
fn render_tree(node: &ContentTreeNode, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    let mut result = format!("{}├─ {} (score: {:.2})\n", indent, node.title, node.score);

    if let Some(ref content) = node.content {
        let preview = if content.len() > 100 {
            format!("{}...", &content[..100])
        } else {
            content.clone()
        };
        result.push_str(&format!("{}│  {}\n", indent, preview.replace('\n', " ")));
    }

    for child in &node.children {
        result.push_str(&render_tree(child, depth + 1));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::NodeId;
    use indextree::Arena;

    fn make_test_node_id() -> NodeId {
        let mut arena = Arena::new();
        let node = crate::domain::TreeNode {
            title: "Test".to_string(),
            structure: String::new(),
            content: String::new(),
            summary: String::new(),
            depth: 0,
            start_index: 0,
            end_index: 0,
            start_page: None,
            end_page: None,
            node_id: None,
            physical_index: None,
            token_count: None,
        };
        NodeId(arena.new_node(node))
    }

    fn make_selected(title: &str, content: &str, score: f32, depth: usize) -> SelectedContent {
        SelectedContent {
            node_id: make_test_node_id(),
            title: title.to_string(),
            content: content.to_string(),
            tokens: 50,
            score,
            depth,
            truncation: None,
        }
    }

    #[test]
    fn test_markdown_builder() {
        let builder = StructureBuilder::new(OutputFormat::Markdown);
        let selected = vec![
            make_selected("Section 1", "Content 1", 0.9, 0),
            make_selected("Section 2", "Content 2", 0.8, 1),
        ];

        // Create a minimal tree for testing
        let tree = DocumentTree::new("Test", "");

        let result = builder.build(selected, &tree);

        assert!(!result.is_empty());
        assert!(result.content.contains("Section 1"));
        assert!(result.content.contains("Section 2"));
        assert!(result.content.contains("# Section 1"));
        assert!(result.content.contains("## Section 2"));
    }

    #[test]
    fn test_flat_builder() {
        let builder = StructureBuilder::new(OutputFormat::Flat);
        let selected = vec![
            make_selected("Section 1", "Content 1", 0.9, 0),
        ];

        let tree = DocumentTree::new("Test", "");
        let result = builder.build(selected, &tree);

        assert!(result.content.contains("[Section 1]"));
        assert!(result.content.contains("Content 1"));
    }

    #[test]
    fn test_builder_with_scores() {
        let builder = StructureBuilder::new(OutputFormat::Markdown)
            .with_scores();

        let selected = vec![
            make_selected("Section 1", "Content 1", 0.95, 0),
        ];

        let tree = DocumentTree::new("Test", "");
        let result = builder.build(selected, &tree);

        assert!(result.content.contains("score: 0.95"));
    }

    #[test]
    fn test_empty_selected() {
        let builder = StructureBuilder::new(OutputFormat::Markdown);
        let tree = DocumentTree::new("Test", "");
        let result = builder.build(Vec::new(), &tree);

        assert!(result.is_empty());
        assert_eq!(result.metadata.node_count, 0);
    }

    #[test]
    fn test_content_tree_node() {
        let mut root = ContentTreeNode::new("Root".to_string())
            .with_content("Root content".to_string(), 0.9);

        let child = ContentTreeNode::new("Child".to_string())
            .with_content("Child content".to_string(), 0.8);

        root.add_child(child);

        assert_eq!(root.children.len(), 1);
        assert_eq!(root.score, 0.9);
    }
}
