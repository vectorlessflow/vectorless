// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Markdown document parser - builds index tree from Markdown header structure.
//!
//! This module provides functionality to parse Markdown documents
//! into a hierarchical tree structure based on header levels.

use async_trait::async_trait;
use regex::Regex;
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

    /// Parse Markdown content and extract nodes.
    fn extract_nodes(&self, content: &str) -> Vec<RawNode> {
        let lines: Vec<&str> = content.lines().collect();

        // Step 1: Extract header nodes (flat list)
        let mut nodes = self.extract_header_nodes(&lines);

        // Return empty result if no headers found
        if nodes.is_empty() {
            return Vec::new();
        }

        // Step 2: Extract text content for each node
        self.extract_node_text(&mut nodes, &lines);

        // Step 3: Calculate token counts (own content only)
        for node in &mut nodes {
            let tokens = estimate_tokens(&node.content);
            node.token_count = Some(tokens);
        }

        nodes
    }

    /// Extract header nodes from Markdown lines.
    fn extract_header_nodes(&self, lines: &[Vec<&str>) -> Vec<RawNode> {
        let header_re = Regex::new(r"^(#{1,6})\s+(.+)$").unwrap();
        let code_block_re = Regex::new(r"^```").unwrap();
        let mut nodes = Vec::new();
        let mut in_code_block = false;

        for (line_idx, line) in lines.iter().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            // Detect code block boundaries
            if code_block_re.is_match(trimmed) {
                in_code_block = !in_code_block;
                continue;
            }

            // Skip empty lines and content inside code blocks
            if trimmed.is_empty() || in_code_block {
                continue;
            }

            // Extract header
            if let Some(caps) = header_re.captures(trimmed) {
                let level = caps[1].len();
                let title = caps[2].trim().to_string();

                if level <= self.config.max_heading_level {
                    nodes.push(RawNode {
                        title,
                        level,
                        line_num,
                        line_end: line_num, // Will be updated later
                        content: String::new(),
                        page: None,
                        token_count: None,
                        total_token_count: None,
                    });
                }
            }
        }

        nodes
    }

    /// Extract text content for each node.
    fn extract_node_text(&self, nodes: &mut [RawNode], lines: &[&str]) {
        for i in 0..nodes.len() {
        let start_line = nodes[i].line_start - 1; // Convert to 0-based

        // Find end position: next same-level or higher-level header
        let end_line = if i + 1 < nodes.len() {
            nodes[i + 1].line_start - 1
        } else {
            lines.len()
        };

        // Extract text (including the header line)
        let text: String = lines[start_line..end_line].join("\n");
        nodes[i].content = text;
        nodes[i].line_end = end_line;
    }
}

}

impl MarkdownParser {
    /// Estimate token count (approximately 1 token per 4 characters).
    fn estimate_tokens(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        (text.len() / 4).max(1)
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

