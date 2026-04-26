// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! PDF document parsing module.
//!
//! This module provides functionality to parse PDF documents:
//! - **PdfPage** — Single page with text and metadata
//! - **PdfParser** — Extract pages from PDF files
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless_compiler::parse::pdf::{PdfParser, PdfPage};
//! use std::path::Path;
//!
//! let parser = PdfParser::new();
//! let result = parser.parse_file(Path::new("document.pdf"))?;
//!
//! println!("Pages: {}", result.pages.len());
//! for page in &result.pages {
//!     println!("Page {}: {} tokens", page.number, page.token_count);
//! }
//! ```

mod parser;
mod types;

pub use parser::PdfParser;
pub use types::PdfPage;

use crate::parse::{Parser, ParseResult};
use std::path::Path;
use vectorless_error::Result;
use vectorless_llm::LlmClient;

/// [`Parser`] trait adapter for [`PdfParser`].
pub struct PdfParserAdapter {
    inner: PdfParser,
}

impl PdfParserAdapter {
    /// Create a PDF parser adapter, optionally with LLM support.
    pub fn new(llm_client: Option<LlmClient>) -> Self {
        let inner = match llm_client {
            Some(client) => PdfParser::with_llm_client(client),
            None => PdfParser::new(),
        };
        Self { inner }
    }
}

#[async_trait::async_trait]
impl Parser for PdfParserAdapter {
    fn name(&self) -> &str { "pdf" }

    fn extensions(&self) -> &[&str] { &["pdf"] }

    async fn parse_content(&self, _content: &str) -> Result<ParseResult> {
        Err(vectorless_error::Error::Parse(
            "PDF requires bytes, not string content".into(),
        ))
    }

    async fn parse_file(&self, path: &Path) -> Result<ParseResult> {
        self.inner.parse_file(path).await
    }

    async fn parse_bytes(&self, data: &[u8]) -> Result<ParseResult> {
        self.inner.parse_bytes_async(data, None).await
    }
}
