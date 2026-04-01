// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! PDF document parser - extracts text from PDF files page by page.
//!
//! This module provides functionality to parse PDF documents
//! into a page-based structure for indexing.

use async_trait::async_trait;
use std::path::Path;

use crate::core::{DocumentParser, Result, Error};

use super::types::{DocumentFormat, DocumentMeta, ParseResult, RawNode};

/// PDF document parser.
#[derive(Debug, Clone, Default)]
pub struct PdfParser;

impl PdfParser {
    /// Create a new PDF parser.
    pub fn new() -> Self {
        Self
    }

    /// Extract text from PDF, returning content per page.
    fn extract_pages(&self, path: &Path) -> Result<Vec<(usize, String)>> {
        let bytes = std::fs::read(path)
            .map_err(|e| Error::Parse(format!("Failed to read PDF file: {}", e)))?;

        // Use pdf-extract to get text
        let text = pdf_extract::extract_text_from_mem(&bytes)
            .map_err(|e| Error::Parse(format!("Failed to extract PDF text: {}", e)))?;

        // For now, treat entire PDF as single content
        // TODO: Use lopdf for page-by-page extraction
        Ok(vec![(1, text)])
    }

    /// Build raw nodes from extracted pages.
    fn build_nodes(&self, pages: &[(usize, String)]) -> Vec<RawNode> {
        let mut nodes = Vec::new();

        // Create root node with document title
        nodes.push(RawNode::new("Document")
            .with_level(0)
            .with_page(1));

        // Create a node for each page
        for (page_num, content) in pages {
            let title = format!("Page {}", page_num);
            let node = RawNode::new(&title)
                .with_content(content.clone())
                .with_level(1)
                .with_page(*page_num)
                .with_lines(*page_num, *page_num);

            nodes.push(node);
        }

        nodes
    }
}

#[async_trait]
impl DocumentParser for PdfParser {
    fn format(&self) -> DocumentFormat {
        DocumentFormat::Pdf
    }

    async fn parse(&self, content: &str) -> Result<ParseResult> {
        // For PDF, content is the file path
        let path = Path::new(content);
        self.parse_file(path).await
    }

    async fn parse_file(&self, path: &Path) -> Result<ParseResult> {
        // Extract text from PDF
        let pages = self.extract_pages(path)?;

        // Build nodes
        let mut nodes = self.build_nodes(&pages);

        // Calculate token counts
        for node in &mut nodes {
            if !node.content.is_empty() {
                node.token_count = Some(estimate_tokens(&node.content));
            }
        }

        // Build metadata
        let name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Document")
            .to_string();

        let meta = DocumentMeta {
            name,
            format: DocumentFormat::Pdf,
            page_count: Some(pages.len()),
            line_count: 0,
            source_path: Some(path.to_string_lossy().to_string()),
            description: None,
        };

        Ok(ParseResult::new(meta, nodes))
    }
}

/// Estimate token count (1 token ≈ 4 characters).
fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    (text.len() / 4).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("hi"), 1);
        assert_eq!(estimate_tokens("hello world"), 1);
        assert_eq!(estimate_tokens(&"a".repeat(100)), 25);
    }
}
