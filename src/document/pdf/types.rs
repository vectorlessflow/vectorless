// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! PDF document types.

use serde::{Deserialize, Serialize};
use crate::token::estimate_tokens;

/// A single page from a PDF document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfPage {
    /// Page number (1-based).
    pub number: usize,

    /// Text content of the page.
    pub text: String,

    /// Estimated token count.
    pub token_count: usize,
}

impl PdfPage {
    /// Create a new PDF page.
    pub fn new(number: usize, text: impl Into<String>) -> Self {
        let text = text.into();
        let token_count = estimate_tokens(&text);
        Self { number, text, token_count }
    }

    /// Check if the page is empty.
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    /// Get character count.
    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    /// Get word count (approximate).
    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }
}

/// PDF document metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfMetadata {
    /// Document title (from metadata or filename).
    pub title: String,

    /// Total page count.
    pub page_count: usize,

    /// Author (if available).
    pub author: Option<String>,

    /// Subject/description (if available).
    pub subject: Option<String>,

    /// Creator application (if available).
    pub creator: Option<String>,

    /// Producer application (if available).
    pub producer: Option<String>,
}

impl Default for PdfMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            page_count: 0,
            author: None,
            subject: None,
            creator: None,
            producer: None,
        }
    }
}

/// Result of parsing a PDF document.
#[derive(Debug, Clone)]
pub struct PdfParseResult {
    /// Document metadata.
    pub metadata: PdfMetadata,

    /// Extracted pages.
    pub pages: Vec<PdfPage>,

    /// Total token count across all pages.
    pub total_tokens: usize,
}

impl PdfParseResult {
    /// Create a new parse result.
    pub fn new(metadata: PdfMetadata, pages: Vec<PdfPage>) -> Self {
        let total_tokens = pages.iter().map(|p| p.token_count).sum();
        Self { metadata, pages, total_tokens }
    }

    /// Check if the document is empty.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Get a page by number (1-based).
    pub fn get_page(&self, number: usize) -> Option<&PdfPage> {
        if number == 0 || number > self.pages.len() {
            return None;
        }
        self.pages.get(number - 1)
    }

    /// Get text for a page range (inclusive, 1-based).
    pub fn get_page_range_text(&self, start: usize, end: usize) -> String {
        let start = start.max(1);
        let end = end.min(self.pages.len());

        self.pages[start - 1..end]
            .iter()
            .map(|p| format!("<page_{}>\n{}\n</page_{}>\n", p.number, p.text, p.number))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_page_creation() {
        let page = PdfPage::new(1, "Hello world");
        assert_eq!(page.number, 1);
        assert_eq!(page.text, "Hello world");
        assert!(page.token_count > 0);
    }

    #[test]
    fn test_estimate_tokens() {
        // Uses tiktoken for accurate counting
        assert_eq!(estimate_tokens(""), 0);
        // "hi" is 1 token in tiktoken
        assert_eq!(estimate_tokens("hi"), 1);
        // tiktoken is efficient at encoding text - just verify it returns a positive count
        let hundred_as = "a".repeat(100);
        assert!(estimate_tokens(&hundred_as) >= 1);
    }

    #[test]
    fn test_page_range_text() {
        let pages = vec![
            PdfPage::new(1, "Page 1 content"),
            PdfPage::new(2, "Page 2 content"),
            PdfPage::new(3, "Page 3 content"),
        ];
        let result = PdfParseResult::new(PdfMetadata::default(), pages);

        let text = result.get_page_range_text(1, 2);
        assert!(text.contains("Page 1 content"));
        assert!(text.contains("Page 2 content"));
        assert!(!text.contains("Page 3 content"));
    }
}
