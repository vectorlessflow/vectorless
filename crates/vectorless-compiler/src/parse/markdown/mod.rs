// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Production-ready Markdown parser module.
//!
//! This module provides a robust Markdown parser built on `pulldown-cmark`,
//! supporting CommonMark, GFM extensions, and frontmatter extraction.
//!
//! # Features
//!
//! - **CommonMark compliant** - Full CommonMark specification support
//! - **GFM extensions** - Tables, strikethrough, task lists, autolinks
//! - **Frontmatter** - YAML and TOML frontmatter parsing
//! - **Configurable** - Fine-grained control over parsing behavior
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless_compiler::parse::markdown::MarkdownParser;
//!
//! let parser = MarkdownParser::new();
//! ```

mod config;
mod frontmatter;
mod parser;

pub use parser::MarkdownParser;

use crate::parse::{Parser, ParseResult};
use std::path::Path;
use vectorless_error::Result;

/// [`Parser`] trait adapter for [`MarkdownParser`].
pub struct MarkdownParserAdapter {
    inner: MarkdownParser,
}

impl MarkdownParserAdapter {
    /// Create a new Markdown parser adapter.
    pub fn new() -> Self {
        Self { inner: MarkdownParser::new() }
    }
}

#[async_trait::async_trait]
impl Parser for MarkdownParserAdapter {
    fn name(&self) -> &str { "markdown" }

    fn extensions(&self) -> &[&str] { &["md", "markdown"] }

    async fn parse_content(&self, content: &str) -> Result<ParseResult> {
        self.inner.parse(content).await
    }

    async fn parse_file(&self, path: &Path) -> Result<ParseResult> {
        self.inner.parse_file(path).await
    }

    async fn parse_bytes(&self, data: &[u8]) -> Result<ParseResult> {
        let content = std::str::from_utf8(data).map_err(|e| {
            vectorless_error::Error::Parse(format!("Invalid UTF-8: {}", e))
        })?;
        self.inner.parse(content).await
    }
}
