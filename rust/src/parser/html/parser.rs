// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! HTML parser implementation using scraper.

use async_trait::async_trait;
use scraper::{ElementRef, Html, Selector};
use std::path::Path;

use crate::error::Result;
use crate::parser::{DocumentFormat, DocumentMeta, DocumentParser, ParseResult, RawNode};
use crate::utils::estimate_tokens;

use super::config::HtmlConfig;

/// Metadata extracted from HTML.
struct HtmlMetadata {
    title: String,
    description: Option<String>,
    author: Option<String>,
    keywords: Option<String>,
}

impl Default for HtmlMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            description: None,
            author: None,
            keywords: None,
        }
    }
}

/// HTML parser that extracts hierarchical structure from HTML documents.
///
/// Uses `scraper` for HTML5-compliant parsing. Extracts heading hierarchy
/// and content from various HTML elements.
#[derive(Debug, Clone)]
pub struct HtmlParser {
    /// Configuration options.
    config: HtmlConfig,
}

impl Default for HtmlParser {
    fn default() -> Self {
        Self::new()
    }
}

impl HtmlParser {
    /// Create a new HTML parser with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(HtmlConfig::default())
    }

    /// Create a parser with custom configuration.
    #[must_use]
    pub fn with_config(config: HtmlConfig) -> Self {
        Self { config }
    }

    /// Parse HTML content and extract nodes.
    fn extract_nodes(&self, content: &str) -> (Vec<RawNode>, HtmlMetadata) {
        let document = Html::parse_document(content);

        // Extract metadata from <head>
        let metadata = self.extract_metadata(&document);

        // Extract nodes from <body>
        let nodes = self.extract_nodes_from_document(&document);

        (nodes, metadata)
    }

    /// Extract metadata from the document head.
    fn extract_metadata(&self, document: &Html) -> HtmlMetadata {
        let mut meta = HtmlMetadata::default();

        // Extract title
        if let Ok(selector) = Selector::parse("title") {
            if let Some(title_elem) = document.select(&selector).next() {
                meta.title = title_elem.text().collect::<String>();
            }
        }

        // Extract meta description
        if let Ok(selector) = Selector::parse("meta[name=\"description\"]") {
            if let Some(desc_elem) = document.select(&selector).next() {
                if let Some(content) = desc_elem.value().attr("content") {
                    meta.description = Some(content.to_string());
                }
            }
        }

        // Extract meta author
        if let Ok(selector) = Selector::parse("meta[name=\"author\"]") {
            if let Some(author_elem) = document.select(&selector).next() {
                if let Some(content) = author_elem.value().attr("content") {
                    meta.author = Some(content.to_string());
                }
            }
        }

        // Extract meta keywords
        if let Ok(selector) = Selector::parse("meta[name=\"keywords\"]") {
            if let Some(keywords_elem) = document.select(&selector).next() {
                if let Some(content) = keywords_elem.value().attr("content") {
                    meta.keywords = Some(content.to_string());
                }
            }
        }

        // Also try Open Graph description
        if meta.description.is_none() {
            if let Ok(selector) = Selector::parse("meta[property=\"og:description\"]") {
                if let Some(og_elem) = document.select(&selector).next() {
                    if let Some(content) = og_elem.value().attr("content") {
                        meta.description = Some(content.to_string());
                    }
                }
            }
        }

        meta
    }

    /// Extract nodes from the document.
    fn extract_nodes_from_document(&self, document: &Html) -> Vec<RawNode> {
        let mut nodes = Vec::new();

        // Parse body selector
        let body_selector = match Selector::parse("body") {
            Ok(s) => s,
            Err(_) => return nodes,
        };

        let body = match document.select(&body_selector).next() {
            Some(b) => b,
            None => return nodes,
        };

        // Collect all headings in order
        let heading_selector = Selector::parse("h1, h2, h3, h4, h5, h6").unwrap();

        let mut headings: Vec<(usize, String, usize)> = Vec::new(); // (index, title, level)

        for (idx, heading) in body.select(&heading_selector).enumerate() {
            let level = self.get_heading_level(heading.value().name());
            if let Some(lvl) = level {
                if lvl <= self.config.max_heading_level {
                    let title: String = heading.text().collect();
                    if !title.trim().is_empty() {
                        headings.push((idx, title.trim().to_string(), lvl));
                    }
                }
            }
        }

        // If no headings found, try to extract content anyway
        if headings.is_empty() {
            let content = self.extract_body_content(body);
            if !content.trim().is_empty() {
                nodes.push(RawNode {
                    title: self.config.default_title.clone(),
                    content: content.trim().to_string(),
                    level: 0,
                    line_start: 1,
                    line_end: 1,
                    page: None,
                    token_count: Some(estimate_tokens(&content)),
                    total_token_count: None,
                });
            }
            return nodes;
        }

        // Extract content between headings
        for (i, (_, title, level)) in headings.iter().enumerate() {
            let content = self.extract_content_after_heading(body, &headings, i);

            if !title.is_empty() || !content.trim().is_empty() {
                nodes.push(RawNode {
                    title: title.clone(),
                    content: content.trim().to_string(),
                    level: *level,
                    line_start: 1,
                    line_end: 1,
                    page: None,
                    token_count: Some(estimate_tokens(&content)),
                    total_token_count: None,
                });
            }
        }

        // Post-process nodes
        self.finalize_nodes(nodes)
    }

    /// Get heading level from tag name (h1-h6).
    fn get_heading_level(&self, tag: &str) -> Option<usize> {
        match tag {
            "h1" => Some(1),
            "h2" => Some(2),
            "h3" => Some(3),
            "h4" => Some(4),
            "h5" => Some(5),
            "h6" => Some(6),
            _ => None,
        }
    }

    /// Extract body content (for documents without headings).
    fn extract_body_content(&self, body: ElementRef) -> String {
        let mut content = String::new();

        // Extract paragraphs
        if let Ok(selector) = Selector::parse("p") {
            for p in body.select(&selector) {
                let text: String = p.text().collect();
                if !text.trim().is_empty() {
                    if !content.is_empty() {
                        content.push_str("\n\n");
                    }
                    content.push_str(text.trim());
                }
            }
        }

        content
    }

    /// Extract content after a heading until the next heading.
    fn extract_content_after_heading(
        &self,
        body: ElementRef,
        headings: &[(usize, String, usize)],
        heading_index: usize,
    ) -> String {
        let mut content = String::new();

        // Get all content elements
        let content_selector = Selector::parse("p, ul, ol, table, pre, blockquote, div.content, article, section")
            .unwrap();

        // This is a simplified approach - extract content from sibling elements
        // In a more sophisticated implementation, we would track DOM positions
        for elem in body.select(&content_selector) {
            let text = self.extract_element_content(elem);
            if !text.is_empty() {
                if !content.is_empty() {
                    content.push_str("\n\n");
                }
                content.push_str(&text);
            }
        }

        content
    }

    /// Extract content from a single element.
    fn extract_element_content(&self, elem: ElementRef) -> String {
        let tag = elem.value().name();

        match tag {
            "p" | "div" | "article" | "section" => {
                let text: String = elem.text().collect();
                text.trim().to_string()
            }
            "ul" => self.extract_list(elem, false),
            "ol" => self.extract_list(elem, true),
            "table" => self.extract_table(elem),
            "pre" | "code" if self.config.include_code_blocks => {
                let text: String = elem.text().collect();
                if !text.trim().is_empty() {
                    format!("```\n{}\n```", text.trim())
                } else {
                    String::new()
                }
            }
            "blockquote" => {
                let text: String = elem.text().collect();
                if !text.trim().is_empty() {
                    text
                        .lines()
                        .map(|line| format!("> {}", line))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        }
    }

    /// Extract list content.
    fn extract_list(&self, element: ElementRef, ordered: bool) -> String {
        let mut result = String::new();
        let li_selector = Selector::parse("li").unwrap();
        let mut counter = 1;

        for li in element.select(&li_selector) {
            let text: String = li.text().collect();
            if !text.trim().is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                if ordered {
                    result.push_str(&format!("{}. {}", counter, text.trim()));
                    counter += 1;
                } else {
                    result.push_str(&format!("• {}", text.trim()));
                }
            }
        }

        result
    }

    /// Extract table content.
    fn extract_table(&self, element: ElementRef) -> String {
        let mut result = String::new();
        let tr_selector = Selector::parse("tr").unwrap();

        for tr in element.select(&tr_selector) {
            let mut cells = Vec::new();
            let td_selector = Selector::parse("td, th").unwrap();

            for cell in tr.select(&td_selector) {
                let text: String = cell.text().collect();
                cells.push(text.trim().to_string());
            }

            if !cells.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(&cells.join(" | "));
            }
        }

        result
    }

    /// Finalize nodes after extraction.
    fn finalize_nodes(&self, mut nodes: Vec<RawNode>) -> Vec<RawNode> {
        // Remove empty nodes
        nodes.retain(|n| !n.title.is_empty() || !n.content.trim().is_empty());

        // Merge small consecutive nodes if configured
        if self.config.merge_small_nodes {
            nodes = self.merge_small_nodes(nodes);
        }

        nodes
    }

    /// Merge small consecutive nodes.
    fn merge_small_nodes(&self, nodes: Vec<RawNode>) -> Vec<RawNode> {
        let mut result: Vec<RawNode> = Vec::new();

        for node in nodes {
            if let Some(last) = result.last_mut() {
                // Merge if same level and content is small
                if last.level == node.level && last.content.len() < self.config.min_content_length
                {
                    if !last.content.is_empty() {
                        last.content.push_str("\n\n");
                    }
                    last.content.push_str(&node.content);
                    continue;
                }
            }
            result.push(node);
        }

        result
    }
}

#[async_trait]
impl DocumentParser for HtmlParser {
    fn format(&self) -> DocumentFormat {
        DocumentFormat::Html
    }

    async fn parse(&self, content: &str) -> Result<ParseResult> {
        let line_count = content.lines().count();
        let (nodes, html_meta) = self.extract_nodes(content);

        let meta = DocumentMeta {
            name: html_meta.title,
            format: DocumentFormat::Html,
            page_count: None,
            line_count,
            source_path: None,
            description: html_meta.description,
        };

        Ok(ParseResult::new(meta, nodes))
    }

    async fn parse_file(&self, path: &Path) -> Result<ParseResult> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| crate::Error::Parse(format!("Failed to read file: {}", e)))?;

        let mut result = self.parse(&content).await?;

        // Extract document name from filename (if not set by meta)
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
    async fn test_parse_simple_html() {
        let parser = HtmlParser::new();
        let html = r#"<html>
            <head><title>Test Document</title></head>
            <body>
                <h1>Main Title</h1>
                <p>This is a paragraph.</p>
                <h2>Section 1</h2>
                <p>Section content.</p>
            </body>
        </html>"#;

        let result = parser.parse(html).await.unwrap();

        assert_eq!(result.meta.name, "Test Document");
        assert!(!result.nodes.is_empty());
    }

    #[tokio::test]
    async fn test_parse_headings() {
        let parser = HtmlParser::new();
        let html = r#"<html><body>
            <h1>H1 Title</h1>
            <p>Content 1</p>
            <h2>H2 Title</h2>
            <p>Content 2</p>
            <h3>H3 Title</h3>
            <p>Content 3</p>
        </body></html>"#;

        let result = parser.parse(html).await.unwrap();

        let heading_nodes: Vec<_> = result.nodes.iter().filter(|n| n.level > 0).collect();
        assert!(heading_nodes.len() >= 3);
    }

    #[tokio::test]
    async fn test_parse_metadata() {
        let parser = HtmlParser::new();
        let html = r#"<html>
            <head>
                <title>My Page</title>
                <meta name="description" content="A test page">
                <meta name="author" content="Test Author">
            </head>
            <body><h1>Content</h1></body>
        </html>"#;

        let result = parser.parse(html).await.unwrap();

        assert_eq!(result.meta.name, "My Page");
        assert_eq!(result.meta.description, Some("A test page".to_string()));
    }

    #[tokio::test]
    async fn test_parse_list() {
        let parser = HtmlParser::new();
        let html = r#"<html><body>
            <h1>List Example</h1>
            <ul>
                <li>Item 1</li>
                <li>Item 2</li>
                <li>Item 3</li>
            </ul>
        </body></html>"#;

        let result = parser.parse(html).await.unwrap();

        let list_node = result.nodes.iter().find(|n| n.title == "List Example");
        assert!(list_node.is_some());
    }

    #[tokio::test]
    async fn test_parse_table() {
        let parser = HtmlParser::new();
        let html = r#"<html><body>
            <h1>Table Example</h1>
            <table>
                <tr><th>Name</th><th>Age</th></tr>
                <tr><td>Alice</td><td>30</td></tr>
            </table>
        </body></html>"#;

        let result = parser.parse(html).await.unwrap();

        let table_node = result.nodes.iter().find(|n| n.title == "Table Example");
        assert!(table_node.is_some());
    }

    #[tokio::test]
    async fn test_empty_document() {
        let parser = HtmlParser::new();
        let result = parser.parse("<html><body></body></html>").await.unwrap();

        assert!(result.nodes.is_empty());
    }

    #[tokio::test]
    async fn test_no_headings() {
        let parser = HtmlParser::new();
        let html = r#"<html><body>
            <p>Just some text.</p>
            <p>More text.</p>
        </body></html>"#;

        let result = parser.parse(html).await.unwrap();

        // Should create a default node
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].title, "Introduction");
    }
}
