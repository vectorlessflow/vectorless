// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document parsing for the index pipeline.
//!
//! Supports Markdown and PDF formats. Parsing is dispatched directly
//! via `match` — no trait objects or registry needed.
//!
//! # Quick parse
//!
//! ```rust,ignore
//! use vectorless::index::parse::{parse_content, parse_bytes, DocumentFormat};
//!
//! let result = parse_content("# Title\nContent", DocumentFormat::Markdown).await?;
//! let result = parse_bytes(&pdf_bytes, DocumentFormat::Pdf).await?;
//! ```

pub mod markdown;
pub mod pdf;
pub mod toc;
pub mod types;

// Re-export core types at module level
pub use types::{DocumentFormat, DocumentMeta, ParseResult, RawNode};

use std::path::Path;

use crate::error::Result;
use crate::index::parse::markdown::MarkdownParser;
use crate::llm::LlmClient;

/// Parse a string content document.
pub async fn parse_content(
    content: &str,
    format: DocumentFormat,
    _llm_client: Option<LlmClient>,
) -> Result<ParseResult> {
    match format {
        DocumentFormat::Markdown => {
            let parser = MarkdownParser::new();
            parser.parse(content).await
        }
        DocumentFormat::Pdf => Err(crate::Error::Parse(
            "PDF requires bytes, not string content".to_string(),
        )),
    }
}

/// Parse a file.
pub async fn parse_file(
    path: &Path,
    format: DocumentFormat,
    llm_client: Option<LlmClient>,
) -> Result<ParseResult> {
    match format {
        DocumentFormat::Markdown => {
            let parser = MarkdownParser::new();
            parser.parse_file(path).await
        }
        DocumentFormat::Pdf => {
            let parser = match llm_client {
                Some(client) => pdf::PdfParser::with_llm_client(client),
                None => pdf::PdfParser::new(),
            };
            parser.parse_file(path).await
        }
    }
}

/// Parse binary data.
pub async fn parse_bytes(
    bytes: &[u8],
    format: DocumentFormat,
    llm_client: Option<LlmClient>,
) -> Result<ParseResult> {
    match format {
        DocumentFormat::Markdown => {
            let content = std::str::from_utf8(bytes)
                .map_err(|e| crate::Error::Parse(format!("Invalid UTF-8 content: {}", e)))?;
            let parser = MarkdownParser::new();
            parser.parse(content).await
        }
        DocumentFormat::Pdf => {
            let parser = match llm_client {
                Some(client) => pdf::PdfParser::with_llm_client(client),
                None => pdf::PdfParser::new(),
            };
            parser.parse_bytes_async(bytes, None).await
        }
    }
}

/// Detect document format from a file extension.
pub fn format_from_extension(ext: &str) -> Option<DocumentFormat> {
    DocumentFormat::from_extension(ext)
}
