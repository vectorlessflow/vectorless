// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Parser registry for managing document parsers.
//!
//! This module provides:
//! - [`ParserRegistry`] - A registry for document parsers with dynamic registration
//! - Module-level functions for quick parsing without registry setup

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::Error;
use crate::error::Result;
use crate::parser::{
    DocumentFormat, DocumentParser, HtmlParser, MarkdownParser, ParseResult, PdfParser,
};

/// Type alias for parser factory functions.
type ParserFactory = Box<dyn Fn() -> Box<dyn DocumentParser> + Send + Sync>;

/// Registry for document parsers.
///
/// Parsers can be registered by format and retrieved at runtime.
///
/// # Example
///
/// ```rust
/// use vectorless::parser::ParserRegistry;
///
/// // Create with default parsers
/// let registry = ParserRegistry::with_defaults();
///
/// // Or create empty and register custom parsers
/// let registry = ParserRegistry::new();
/// ```
pub struct ParserRegistry {
    /// Registered parser factories by format.
    factories: Arc<RwLock<HashMap<DocumentFormat, ParserFactory>>>,
}

impl std::fmt::Debug for ParserRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let factories = self.factories.read().unwrap();
        let formats: Vec<_> = factories.keys().collect();
        f.debug_struct("ParserRegistry")
            .field("formats", &formats)
            .finish()
    }
}

impl ParserRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            factories: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a registry with default parsers.
    pub fn with_defaults() -> Self {
        let registry = Self::new();
        registry.register_defaults();
        registry
    }

    /// Register default parsers (Markdown, PDF, HTML, DOCX).
    pub fn register_defaults(&self) {
        self.register("markdown", || Box::new(MarkdownParser::new()));
        self.register("pdf", || Box::new(PdfParser::new()));
        self.register("html", || Box::new(HtmlParser::new()));
        self.register("docx", || Box::new(super::docx::DocxParser::new()));
    }

    /// Register a parser factory by name.
    pub fn register<F>(&self, name: &str, factory: F)
    where
        F: Fn() -> Box<dyn DocumentParser> + Send + Sync + 'static,
    {
        // Create a temporary parser to get the format
        let parser = factory();
        let format = parser.format();

        let mut factories = self.factories.write().unwrap();
        factories.insert(format, Box::new(factory));

        let _ = name; // Name is for documentation purposes
    }

    /// Get a parser by format.
    pub fn get(&self, format: DocumentFormat) -> Option<Box<dyn DocumentParser>> {
        let factories = self.factories.read().unwrap();
        factories.get(&format).map(|f| f())
    }

    /// Check if a format is supported.
    pub fn supports(&self, format: DocumentFormat) -> bool {
        let factories = self.factories.read().unwrap();
        factories.contains_key(&format)
    }

    /// List supported formats.
    pub fn supported_formats(&self) -> Vec<DocumentFormat> {
        let factories = self.factories.read().unwrap();
        factories.keys().copied().collect()
    }

    /// Parse content using the appropriate parser.
    pub async fn parse(&self, content: &str, format: DocumentFormat) -> Result<ParseResult> {
        let parser = self
            .get(format)
            .ok_or_else(|| Error::Parse(format!("Unsupported format: {:?}", format)))?;
        parser.parse(content).await
    }

    /// Parse a file using the appropriate parser.
    pub async fn parse_file(&self, path: &Path) -> Result<ParseResult> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| Error::Parse("Could not determine file extension".to_string()))?;

        let format = DocumentFormat::from_extension(ext)
            .ok_or_else(|| Error::Parse(format!("Unknown format: {}", ext)))?;

        self.parse_file_as(path, format).await
    }

    /// Parse a file with a specific format.
    pub async fn parse_file_as(&self, path: &Path, format: DocumentFormat) -> Result<ParseResult> {
        let parser = self
            .get(format)
            .ok_or_else(|| Error::Parse(format!("Unsupported format: {:?}", format)))?;
        parser.parse_file(path).await
    }

    /// Parse binary data using the appropriate parser.
    ///
    /// For text-based formats, the bytes are converted to UTF-8 string first.
    /// For binary formats (PDF, DOCX), the parser handles the bytes directly.
    pub async fn parse_bytes(&self, bytes: &[u8], format: DocumentFormat) -> Result<ParseResult> {
        match format {
            DocumentFormat::Markdown | DocumentFormat::Html => {
                // Text formats - convert to string first
                let content = std::str::from_utf8(bytes)
                    .map_err(|e| Error::Parse(format!("Invalid UTF-8 content: {}", e)))?;
                self.parse(content, format).await
            }
            DocumentFormat::Pdf | DocumentFormat::Docx => {
                // Binary formats - write to temp file and parse
                // This is a temporary solution until parsers support bytes directly
                let temp_dir = std::env::temp_dir();
                let ext = format.extension();
                let temp_file =
                    temp_dir.join(format!("vectorless_temp_{}.{}", uuid::Uuid::new_v4(), ext));

                std::fs::write(&temp_file, bytes)
                    .map_err(|e| Error::Parse(format!("Failed to write temp file: {}", e)))?;

                let result = self.parse_file_as(&temp_file, format).await;

                // Clean up temp file
                let _ = std::fs::remove_file(&temp_file);

                result
            }
        }
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// =============================================================================
// Module-level convenience functions
// =============================================================================

/// Get a parser for the given format.
///
/// Returns `None` if the format is not supported.
pub fn get_parser(format: DocumentFormat) -> Option<Box<dyn DocumentParser>> {
    match format {
        DocumentFormat::Markdown => Some(Box::new(MarkdownParser::new())),
        DocumentFormat::Pdf => Some(Box::new(PdfParser::new())),
        DocumentFormat::Html => Some(Box::new(HtmlParser::new())),
        DocumentFormat::Docx => Some(Box::new(super::docx::DocxParser::new())),
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
        .ok_or_else(|| Error::Parse(format!("Unsupported format: {:?}", format)))?;
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
        .ok_or_else(|| Error::Parse(format!("Unsupported file: {:?}", path)))?;
    parser.parse_file(path).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_defaults() {
        let registry = ParserRegistry::with_defaults();
        assert!(registry.supports(DocumentFormat::Markdown));
    }

    #[test]
    fn test_supported_formats() {
        let registry = ParserRegistry::with_defaults();
        let formats = registry.supported_formats();
        assert!(formats.contains(&DocumentFormat::Markdown));
        assert!(formats.contains(&DocumentFormat::Html));
    }

    #[test]
    fn test_get_parser() {
        let registry = ParserRegistry::with_defaults();
        let parser = registry.get(DocumentFormat::Markdown);
        assert!(parser.is_some());
    }

    #[test]
    fn test_unsupported_format() {
        let registry = ParserRegistry::new(); // Empty registry
        let parser = registry.get(DocumentFormat::Pdf);
        assert!(parser.is_none());
    }

    #[test]
    fn test_pdf_parser_registered() {
        let registry = ParserRegistry::with_defaults();
        assert!(registry.supports(DocumentFormat::Pdf));
        let parser = registry.get(DocumentFormat::Pdf);
        assert!(parser.is_some());
    }

    #[test]
    fn test_html_parser_registered() {
        let registry = ParserRegistry::with_defaults();
        assert!(registry.supports(DocumentFormat::Html));
        let parser = registry.get(DocumentFormat::Html);
        assert!(parser.is_some());
    }

    #[test]
    fn test_get_parser_function() {
        let parser = get_parser(DocumentFormat::Markdown);
        assert!(parser.is_some());
    }

    #[test]
    fn test_get_parser_for_file() {
        let parser = get_parser_for_file(Path::new("test.md"));
        assert!(parser.is_some());
    }

    #[test]
    fn test_get_html_parser_for_file() {
        let parser = get_parser_for_file(Path::new("test.html"));
        assert!(parser.is_some());
    }
}
