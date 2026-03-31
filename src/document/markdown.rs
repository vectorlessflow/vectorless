// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Markdown document parser.
//!
//! This module provides a parser for Markdown documents that extracts
//! hierarchical sections based on headings (#, ##, ###, etc.).
//!
//! # Example
//!
//! ```rust
//! use vectorless::document::{DocumentParser, MarkdownParser};
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::core::Result<()> {
//! let parser = MarkdownParser::new();
//! let content = r#"
//! # Main Title
//!
//! Introduction paragraph.
//!
//! ## Section 1
//!
//! Content for section 1.
//!
//! ## Section 2
//!
//! Content for section 2.
//! "#;
//!
//! let result = parser.parse(content).await?;
//! println!("Extracted {} nodes", result.node_count());
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use pulldown_cmark::{Event, HeadingLevel, Parser as CmarkParser, Tag, TagEnd};
use std::path::Path;

use crate::core::DocumentParser;
use crate::core::Result;

use super::types::{DocumentFormat, DocumentMeta, ParseResult, RawNode};

/// Configuration for the Markdown parser.
#[derive(Debug, Clone)]
pub struct MarkdownConfig {
    /// Include code blocks in content.
    pub include_code_blocks: bool,

    /// Maximum heading level to parse (1-6).
    pub max_heading_level: usize,

    /// Extract frontmatter as metadata.
    pub parse_frontmatter: bool,
}

impl Default for MarkdownConfig {
    fn default() -> Self {
        Self {
            include_code_blocks: true,
            max_heading_level: 6,
            parse_frontmatter: true,
        }
    }
}

/// Markdown document parser.
#[derive(Debug, Clone)]
pub struct MarkdownParser {
    config: MarkdownConfig,
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownParser {
    /// Create a new Markdown parser with default configuration.
    pub fn new() -> Self {
        Self {
            config: MarkdownConfig::default(),
        }
    }

    /// Create a new Markdown parser with custom configuration.
    pub fn with_config(config: MarkdownConfig) -> Self {
        Self { config }
    }

    /// Parse the markdown content and extract nodes.
    fn extract_nodes(&self, content: &str) -> Vec<RawNode> {
        let mut nodes: Vec<RawNode> = Vec::new();
        let mut current_node: Option<RawNode> = None;
        let mut current_content = String::new();
        let mut in_code_block = false;

        // Track line positions
        let lines: Vec<&str> = content.lines().collect();
        let parser = CmarkParser::new(content);

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    // Save previous node if exists
                    if let Some(mut node) = current_node.take() {
                        node.content = current_content.trim().to_string();
                        if node.has_content() || !node.title.is_empty() {
                            nodes.push(node);
                        }
                    }
                    current_content.clear();

                    // Create new node based on heading level
                    let heading_level = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };

                    if heading_level as usize <= self.config.max_heading_level {
                        current_node = Some(RawNode {
                            level: heading_level as usize,
                            ..Default::default()
                        });
                    }
                }

                Event::End(TagEnd::Heading(_)) => {
                    // Title is captured in Text events
                }

                Event::Start(Tag::CodeBlock(_)) => {
                    in_code_block = true;
                    if self.config.include_code_blocks {
                        current_content.push_str("```\n");
                    }
                }

                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;
                    if self.config.include_code_blocks {
                        current_content.push_str("\n```\n");
                    }
                }

                Event::Text(text) => {
                    if current_node.is_none() {
                        // Text before any heading - create a root node
                        current_node = Some(RawNode {
                            level: 0,
                            ..Default::default()
                        });
                    }

                    if let Some(ref mut node) = current_node {
                        if node.title.is_empty() && node.level > 0 {
                            // This is the heading text
                            node.title = text.to_string();
                        } else if in_code_block && self.config.include_code_blocks {
                            current_content.push_str(&text);
                        } else if !in_code_block {
                            current_content.push_str(&text);
                        }
                    }
                }

                Event::SoftBreak | Event::HardBreak => {
                    current_content.push('\n');
                }

                _ => {}
            }
        }

        // Save the last node
        if let Some(mut node) = current_node {
            node.content = current_content.trim().to_string();
            if node.has_content() || !node.title.is_empty() {
                nodes.push(node);
            }
        }

        // Post-process: calculate line positions more accurately
        self.calculate_line_positions(&mut nodes, &lines);

        nodes
    }

    /// Calculate accurate line positions for each node.
    fn calculate_line_positions(&self, nodes: &mut [RawNode], lines: &[&str]) {
        // Find heading lines
        let mut heading_lines: Vec<(usize, usize)> = Vec::new(); // (line_idx, level)

        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                let level = trimmed.chars().take_while(|&c| c == '#').count();
                if level <= 6 && (trimmed.len() == level || trimmed.chars().nth(level) == Some(' ')) {
                    heading_lines.push((idx + 1, level)); // 1-based line number
                }
            }
        }

        // Match nodes with heading lines
        let mut heading_idx = 0;
        for node in nodes.iter_mut() {
            // Find the next heading that matches this node's level
            while heading_idx < heading_lines.len() {
                let (line, level) = heading_lines[heading_idx];
                if level == node.level {
                    node.line_start = line;
                    heading_idx += 1;
                    break;
                }
                heading_idx += 1;
            }

            // Set line_end to the line before the next same-level or higher-level heading
            let end_line = if heading_idx < heading_lines.len() {
                heading_lines[heading_idx].0.saturating_sub(1)
            } else {
                lines.len()
            };
            node.line_end = end_line;
        }
    }
}

#[async_trait]
impl DocumentParser for MarkdownParser {
    fn format(&self) -> DocumentFormat {
        DocumentFormat::Markdown
    }

    async fn parse(&self, content: &str) -> Result<ParseResult> {
        let line_count = content.lines().count();
        let nodes = self.extract_nodes(content);

        let meta = DocumentMeta {
            name: String::new(),
            format: DocumentFormat::Markdown,
            page_count: None,
            line_count,
            source_path: None,
            description: None,
        };

        Ok(ParseResult::new(meta, nodes))
    }

    async fn parse_file(&self, path: &Path) -> Result<ParseResult> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| crate::core::Error::Parse(format!("Failed to read file: {}", e)))?;

        let mut result = self.parse(&content).await?;

        // Extract document name from filename
        if let Some(stem) = path.file_stem() {
            result.meta.name = stem.to_string_lossy().to_string();
        }
        result.meta.source_path = Some(path.to_string_lossy().to_string());

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_simple() {
        let parser = MarkdownParser::new();
        let content = "# Title\n\nContent here.";
        let result = parser.parse(content).await.unwrap();

        assert_eq!(result.node_count(), 1);
        assert_eq!(result.nodes[0].title, "Title");
        assert_eq!(result.nodes[0].level, 1);
    }

    #[tokio::test]
    async fn test_parse_nested() {
        let parser = MarkdownParser::new();
        let content = r#"
# Main

## Section 1

Content 1.

## Section 2

Content 2.
"#;
        let result = parser.parse(content).await.unwrap();

        assert!(result.node_count() >= 3);
    }

    #[tokio::test]
    async fn test_parse_code_blocks() {
        let parser = MarkdownParser::new();
        let content = r#"
# Code Example

```rust
fn main() {
    println!("Hello");
}
```
"#;
        let result = parser.parse(content).await.unwrap();

        assert!(result.nodes[0].content.contains("fn main"));
    }
}
