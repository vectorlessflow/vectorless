// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Main Markdown parser implementation.

use async_trait::async_trait;
use pulldown_cmark::Options;
use std::path::Path;

use crate::core::Result;
use crate::parser::{DocumentFormat, DocumentMeta, DocumentParser, ParseResult, RawNode};
use crate::core::token::estimate_tokens;

use super::config::MarkdownConfig;
use super::frontmatter;

/// Production-ready Markdown parser.
///
/// Built on `pulldown-cmark` for robust CommonMark/GFM parsing.
///
/// # Features
///
/// - CommonMark compliant
/// - GitHub Flavored Markdown (GFM) extensions
/// - YAML/TOML frontmatter extraction
/// - Configurable parsing behavior
///
/// # Example
///
/// ```rust
/// use vectorless::parser::markdown::MarkdownParser;
/// use vectorless::parser::DocumentParser;
///
/// # #[tokio::main]
/// # async fn main() -> vectorless::core::Result<()> {
/// let parser = MarkdownParser::new();
/// let result = parser.parse("# Title\n\nContent").await?;
///
/// println!("Found {} nodes", result.node_count());
/// # Ok(())
/// # }
/// ```
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
    /// Create a new parser with default (GFM) configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(MarkdownConfig::default())
    }

    /// Create a parser with custom configuration.
    #[must_use]
    pub fn with_config(config: MarkdownConfig) -> Self {
        Self { config }
    }

    /// Build pulldown-cmark options from configuration.
    fn build_options(&self) -> Options {
        let mut options = Options::empty();

        // GFM extensions
        if self.config.enable_gfm {
            options.insert(Options::ENABLE_TABLES);
            options.insert(Options::ENABLE_STRIKETHROUGH);
            options.insert(Options::ENABLE_TASKLISTS);
            options.insert(Options::ENABLE_SMART_PUNCTUATION);
        }

        // Footnotes
        if self.config.enable_footnotes {
            options.insert(Options::ENABLE_FOOTNOTES);
        }

        // Definition lists
        if self.config.enable_definition_lists {
            options.insert(Options::ENABLE_DEFINITION_LIST);
        }

        // Note: pulldown-cmark 0.12 doesn't have ENABLE_SUPERSCRIPT/ENABLE_SUBSCRIPT
        // Super/subscript handling would require custom processing if needed

        options
    }

    /// Parse Markdown content and extract nodes.
    fn extract_nodes(&self, content: &str) -> (Vec<RawNode>, Option<std::collections::HashMap<String, String>>) {
        // 1. Extract frontmatter (if present)
        let (fm, remaining_content) = frontmatter::extract_frontmatter(
            content,
            self.config.parse_frontmatter,
            self.config.parse_toml_frontmatter,
        );

        // 2. Build parser options
        let options = self.build_options();

        // 3. Parse with pulldown-cmark
        let parser = pulldown_cmark::Parser::new_ext(remaining_content, options);

        // 4. Extract raw nodes from events
        let nodes = self.extract_nodes_from_events(parser);

        // 5. Extract frontmatter fields
        let fm_fields = fm.map(|f| {
            self.config
                .frontmatter_fields
                .iter()
                .filter_map(|field| f.get(field).map(|v| (field.clone(), v.clone())))
                .collect()
        });

        (nodes, fm_fields)
    }

    /// Extract RawNodes from pulldown-cmark event iterator.
    fn extract_nodes_from_events<'a, E>(&self, events: E) -> Vec<RawNode>
    where
        E: Iterator<Item = pulldown_cmark::Event<'a>>,
    {
        use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};

        let mut nodes: Vec<RawNode> = Vec::new();
        let mut current: Option<InProgressNode> = None;
        let mut content_buffer = String::new();
        let mut title_buffer = String::new();
        let mut preamble_content = String::new();
        let mut current_line: usize = 1;
        let mut in_heading = false;
        let mut skip_content = false;

        for event in events {
            match event {
                Event::Start(tag) => match tag {
                    Tag::Heading { level, .. } => {
                        let level_num = level as usize;

                        // Check if this heading level should be processed as a node
                        if level_num > self.config.max_heading_level
                            || level_num < self.config.min_heading_level
                        {
                            // Treat as content - add the heading marker to content
                            in_heading = false;
                            skip_content = false;
                            content_buffer.push_str(&format!("{} ", "#".repeat(level_num)));
                            continue;
                        }

                        // Finish any current node first
                        if let Some(node) = finish_current_node(
                            &mut current,
                            &mut content_buffer,
                            &mut preamble_content,
                            &mut nodes,
                            &self.config,
                            current_line,
                        ) {
                            nodes.push(node);
                        }

                        // Start new heading
                        in_heading = true;
                        title_buffer.clear();

                        current = Some(InProgressNode {
                            title: String::new(),
                            level: level_num,
                            line_start: current_line,
                        });
                    }
                    Tag::CodeBlock(kind) => {
                        if self.config.include_code_blocks {
                            match kind {
                                CodeBlockKind::Fenced(lang) => {
                                    content_buffer.push_str("\n```");
                                    content_buffer.push_str(&lang);
                                    content_buffer.push('\n');
                                }
                                CodeBlockKind::Indented => {
                                    content_buffer.push_str("\n```\n");
                                }
                            }
                        } else {
                            skip_content = true;
                        }
                    }
                    _ => {}
                },
                Event::End(tag) => match tag {
                    TagEnd::Heading(_) => {
                        if in_heading {
                            in_heading = false;
                            if let Some(ref mut node) = current {
                                node.title = title_buffer.trim().to_string();
                                title_buffer.clear();

                                if node.title.chars().count() < self.config.min_heading_chars {
                                    current = None;
                                }
                            }
                        }
                    }
                    TagEnd::CodeBlock => {
                        skip_content = false;
                        if self.config.include_code_blocks {
                            content_buffer.push_str("\n```\n");
                        }
                    }
                    _ => {}
                },
                Event::Text(text) => {
                    current_line += text.chars().filter(|&c| c == '\n').count();

                    if in_heading {
                        title_buffer.push_str(&text);
                    } else if !skip_content {
                        content_buffer.push_str(&text);
                    }
                }
                Event::Code(code) => {
                    if !in_heading && !skip_content {
                        content_buffer.push('`');
                        content_buffer.push_str(&code);
                        content_buffer.push('`');
                    }
                }
                Event::Html(html) | Event::InlineHtml(html) => {
                    if !skip_content {
                        content_buffer.push_str(&html);
                        current_line += html.chars().filter(|&c| c == '\n').count();
                    }
                }
                Event::SoftBreak => {
                    if !skip_content {
                        content_buffer.push(' ');
                    }
                }
                Event::HardBreak => {
                    if !skip_content {
                        content_buffer.push('\n');
                        current_line += 1;
                    }
                }
                Event::Rule => {
                    if !skip_content {
                        content_buffer.push_str("\n\n---\n\n");
                    }
                }
                _ => {}
            }
        }

        // Finish any remaining node
        if let Some(node) = finish_current_node(
            &mut current,
            &mut content_buffer,
            &mut preamble_content,
            &mut nodes,
            &self.config,
            current_line,
        ) {
            nodes.push(node);
        }

        // Handle document with no headings (only preamble)
        if nodes.is_empty()
            && self.config.create_preamble_node
            && (!content_buffer.trim().is_empty() || !preamble_content.is_empty())
        {
            // Use preamble_content if available, otherwise use content_buffer
            let content = if !preamble_content.is_empty() {
                preamble_content.trim()
            } else {
                content_buffer.trim()
            };
            nodes.push(RawNode {
                title: self.config.preamble_title.clone(),
                level: 0,
                content: content.to_string(),
                line_start: 1,
                line_end: current_line,
                page: None,
                token_count: Some(estimate_tokens(content)),
                total_token_count: None,
            });
        }

        nodes
    }
}

/// In-progress node being constructed.
struct InProgressNode {
    title: String,
    level: usize,
    line_start: usize,
}

/// Finish the current node and return it if valid.
#[allow(clippy::too_many_arguments)]
fn finish_current_node(
    current: &mut Option<InProgressNode>,
    content_buffer: &mut String,
    preamble_content: &mut String,
    nodes: &mut Vec<RawNode>,
    config: &MarkdownConfig,
    current_line: usize,
) -> Option<RawNode> {
    // Handle preamble content
    if nodes.is_empty() && !content_buffer.trim().is_empty() {
        if config.create_preamble_node {
            let content = content_buffer.trim();
            *preamble_content = content.to_string();
        }
    }

    // Finish current heading node
    if let Some(node) = current.take() {
        let content = content_buffer.trim().to_string();

        // If this is the first heading and we have preamble content,
        // prepend it to this node's content
        let final_content = if nodes.is_empty() && !preamble_content.is_empty() {
            let combined = format!("{}\n\n{}", preamble_content, content);
            preamble_content.clear();
            combined
        } else {
            content
        };

        content_buffer.clear();

        return Some(RawNode {
            title: node.title,
            level: node.level,
            content: final_content.trim().to_string(),
            line_start: node.line_start,
            line_end: current_line,
            page: None,
            token_count: Some(estimate_tokens(&final_content)),
            total_token_count: None,
        });
    }

    content_buffer.clear();
    None
}

#[async_trait]
impl DocumentParser for MarkdownParser {
    fn format(&self) -> DocumentFormat {
        DocumentFormat::Markdown
    }

    async fn parse(&self, content: &str) -> Result<ParseResult> {
        let line_count = content.lines().count();
        let (nodes, fm_fields) = self.extract_nodes(content);

        // Build metadata
        let mut meta = DocumentMeta {
            name: String::new(),
            format: DocumentFormat::Markdown,
            page_count: None,
            line_count,
            source_path: None,
            description: None,
        };

        // Apply frontmatter fields
        if let Some(fields) = fm_fields {
            if let Some(title) = fields.get("title") {
                meta.name = title.clone();
            }
            if let Some(desc) = fields.get("description") {
                meta.description = Some(desc.clone());
            }
        }

        Ok(ParseResult::new(meta, nodes))
    }

    async fn parse_file(&self, path: &Path) -> Result<ParseResult> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| crate::core::Error::Parse(format!("Failed to read file: {}", e)))?;

        let mut result = self.parse(&content).await?;

        // Extract document name from filename (if not set by frontmatter)
        if result.meta.name.is_empty() {
            if let Some(stem) = path.file_stem() {
                result.meta.name = stem.to_string_lossy().to_string();
            }
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

        assert!(!result.nodes.is_empty());
        assert!(result.nodes.iter().any(|n| n.title == "Title" && n.level == 1));
    }

    #[tokio::test]
    async fn test_parse_nested() {
        let parser = MarkdownParser::new();
        let content = r#"# Main

## Section 1

Content 1.

## Section 2

Content 2."#;
        let result = parser.parse(content).await.unwrap();

        let heading_nodes: Vec<_> = result.nodes.iter().filter(|n| n.level > 0).collect();
        assert!(heading_nodes.len() >= 3);
    }

    #[tokio::test]
    async fn test_parse_code_blocks() {
        let parser = MarkdownParser::new();
        let content = r#"# Code Example

```rust
fn main() {
    println!("Hello");
}
```"#;
        let result = parser.parse(content).await.unwrap();

        // Should have the heading node
        let heading_node = result.nodes.iter().find(|n| n.title == "Code Example");
        assert!(heading_node.is_some());

        // Code block should be in content
        assert!(heading_node.unwrap().content.contains("```rust"));
    }

    #[tokio::test]
    async fn test_skip_headers_in_code_blocks() {
        let parser = MarkdownParser::new();
        let content = r#"# Title 1

Content before code.

```
# This is not a header
# Also not a header
```

## Title 1.1

Content after code."#;

        let result = parser.parse(content).await.unwrap();

        // Should only have Title 1 and Title 1.1 as heading nodes
        let heading_titles: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.level > 0)
            .map(|n| n.title.as_str())
            .collect();

        assert!(heading_titles.contains(&"Title 1"));
        assert!(heading_titles.contains(&"Title 1.1"));
        assert!(!heading_titles.contains(&"This is not a header"));
    }

    #[tokio::test]
    async fn test_frontmatter_extraction() {
        let parser = MarkdownParser::new();
        let content = r#"---
title: My Document
description: A test document
---

# Content

Body text."#;

        let result = parser.parse(content).await.unwrap();

        assert_eq!(result.meta.name, "My Document");
        assert_eq!(result.meta.description, Some("A test document".to_string()));
    }

    #[tokio::test]
    async fn test_gfm_table() {
        let parser = MarkdownParser::new();
        let content = r#"# Table Example

| Name | Age |
|------|-----|
| Alice | 30 |
| Bob | 25 |"#;

        let result = parser.parse(content).await.unwrap();

        let table_node = result.nodes.iter().find(|n| n.title == "Table Example");
        assert!(table_node.is_some());
        assert!(table_node.unwrap().content.contains("Alice"));
    }

    #[tokio::test]
    async fn test_max_heading_level_config() {
        let config = MarkdownConfig {
            max_heading_level: 2,
            ..Default::default()
        };
        let parser = MarkdownParser::with_config(config);

        let content = r#"# H1

## H2

### H3

#### H4"#;

        let result = parser.parse(content).await.unwrap();

        // H3 and H4 should not be separate nodes
        let heading_nodes: Vec<_> = result.nodes.iter().filter(|n| n.level > 0).collect();
        assert_eq!(heading_nodes.len(), 2);
    }

    #[tokio::test]
    async fn test_no_code_blocks_config() {
        let config = MarkdownConfig::no_code_blocks();
        let parser = MarkdownParser::with_config(config);

        let content = r#"# Example

```rust
let x = 1;
```

Some text."#;

        let result = parser.parse(content).await.unwrap();

        let node = result.nodes.iter().find(|n| n.title == "Example").unwrap();
        // Code block should not be in content
        assert!(!node.content.contains("let x = 1"));
        // But regular text should be
        assert!(node.content.contains("Some text"));
    }

    #[tokio::test]
    async fn test_empty_document() {
        let parser = MarkdownParser::new();
        let result = parser.parse("").await.unwrap();

        assert!(result.nodes.is_empty());
    }

    #[tokio::test]
    async fn test_document_with_no_headings() {
        let parser = MarkdownParser::new();
        let content = "Just some text\nwith no headings.";

        let result = parser.parse(content).await.unwrap();

        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].title, "Introduction");
        assert_eq!(result.nodes[0].level, 0);
    }
}
