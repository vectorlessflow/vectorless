// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document parser utilities and factory functions.
//!
//! This module provides helper functions for working with document parsers.
//! The [`DocumentParser`] trait is defined in [`crate::core::traits`].

use std::path::Path;

use super::types::{DocumentFormat, ParseResult};
use crate::core::{DocumentParser, Result};

/// Get a parser for the given format.
///
/// Returns `None` if the format is not supported.
pub fn get_parser(format: DocumentFormat) -> Option<Box<dyn DocumentParser>> {
    match format {
        DocumentFormat::Markdown => Some(Box::new(super::markdown::MarkdownParser::new())),
        DocumentFormat::Pdf => Some(Box::new(super::pdf::PdfParser::new())),
        DocumentFormat::Html => {
            // TODO: Implement HTML parser
            None
        }
        DocumentFormat::Docx => Some(Box::new(super::docx::DocxParser::new())),
        DocumentFormat::Text => {
            // TODO: Implement plain text parser
            None
        }
    }
}

/// Get a parser for a file based on its extension.
///
/// Returns `None` if the extension is not recognized or not supported.
pub fn get_parser_for_file(path: &Path) -> Option<Box<dyn DocumentParser>> {
    let ext = path.extension()?.to_str()?;
    let format = DocumentFormat::from_extension(ext)?;
    get_parser(format)
}

/// Parse a document from content using the appropriate parser.
///
/// # Arguments
///
/// * `content` - The document content
/// * `format` - The document format
///
/// # Returns
///
/// A [`ParseResult`] containing the extracted nodes.
pub async fn parse_content(content: &str, format: DocumentFormat) -> Result<ParseResult> {
    let parser = get_parser(format)
        .ok_or_else(|| crate::core::Error::Parse(format!("Unsupported format: {:?}", format)))?;
    parser.parse(content).await
}

/// Parse a document from a file.
///
/// # Arguments
///
/// * `path` - Path to the file
///
/// # Returns
///
/// A [`ParseResult`] containing the extracted nodes.
pub async fn parse_file(path: &Path) -> Result<ParseResult> {
    let parser = get_parser_for_file(path)
        .ok_or_else(|| crate::core::Error::Parse(format!("Unsupported file: {:?}", path)))?;
    parser.parse_file(path).await
}
