// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document parsing for the compile pipeline.
//!
//! Supports Markdown and PDF formats out of the box. Custom parsers can be
//! added via the [`Parser`] trait and [`ParserRegistry`].
//!
//! # Adding a custom parser
//!
//! ```rust,ignore
//! use vectorless_compiler::parse::{Parser, ParseResult, ParserRegistry};
//!
//! struct MyParser;
//!
//! #[async_trait]
//! impl Parser for MyParser {
//!     fn name(&self) -> &str { "my-format" }
//!     fn extensions(&self) -> &[&str] { &["foo", "bar"] }
//!     async fn parse_content(&self, content: &str) -> Result<ParseResult> { ... }
//!     async fn parse_file(&self, path: &Path) -> Result<ParseResult> { ... }
//! }
//!
//! let registry = ParserRegistry::default_parsers(None).with(MyParser);
//! ```

pub mod markdown;
pub mod pdf;
pub mod toc;
pub mod types;

// Re-export core types at module level
pub use types::{DocumentFormat, DocumentMeta, ParseResult, RawNode};

use std::collections::HashMap;
use std::path::Path;

use crate::parse::markdown::MarkdownParser;
use vectorless_error::Result;
use vectorless_llm::LlmClient;

// ---------------------------------------------------------------------------
// Parser trait
// ---------------------------------------------------------------------------

/// Trait for document format parsers.
///
/// Implement this to add support for a new document format.
/// Register via [`ParserRegistry::register`] or [`ParserRegistry::with`].
#[async_trait::async_trait]
pub trait Parser: Send + Sync {
    /// Parser name (e.g., "markdown", "pdf", "code").
    fn name(&self) -> &str;

    /// File extensions this parser handles, without dot (e.g., `["py", "rs"]`).
    fn extensions(&self) -> &[&str] {
        &[]
    }

    /// Parse string content into raw nodes.
    async fn parse_content(&self, content: &str) -> Result<ParseResult>;

    /// Parse a file into raw nodes.
    async fn parse_file(&self, path: &Path) -> Result<ParseResult>;

    /// Parse binary data into raw nodes.
    async fn parse_bytes(&self, data: &[u8]) -> Result<ParseResult> {
        let _ = data;
        Err(vectorless_error::Error::Parse(
            "Binary parsing not supported by this parser".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// ParserRegistry
// ---------------------------------------------------------------------------

/// Registry of document format parsers.
///
/// Maps parser names and file extensions to [`Parser`] implementations.
/// Built-in parsers for Markdown and PDF are provided by [`ParserRegistry::default_parsers`].
pub struct ParserRegistry {
    parsers: HashMap<String, Box<dyn Parser>>,
    extension_map: HashMap<String, String>,
}

impl ParserRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
            extension_map: HashMap::new(),
        }
    }

    /// Register a parser. Extensions declared by the parser are auto-indexed.
    pub fn register(&mut self, parser: impl Parser + 'static) {
        let name = parser.name().to_string();
        for ext in parser.extensions() {
            self.extension_map.insert(ext.to_lowercase(), name.clone());
        }
        self.parsers.insert(name, Box::new(parser));
    }

    /// Builder-style registration.
    pub fn with(mut self, parser: impl Parser + 'static) -> Self {
        self.register(parser);
        self
    }

    /// Get a parser by name.
    pub fn get(&self, name: &str) -> Option<&dyn Parser> {
        self.parsers.get(name).map(|p| p.as_ref())
    }

    /// Get a parser by file extension (lowercase).
    pub fn get_by_extension(&self, ext: &str) -> Option<&dyn Parser> {
        self.extension_map
            .get(&ext.to_lowercase())
            .and_then(|name| self.parsers.get(name))
            .map(|p| p.as_ref())
    }

    /// Default registry with built-in Markdown + PDF parsers.
    pub fn default_parsers(llm_client: Option<LlmClient>) -> Self {
        let mut registry = Self::new();
        registry.register(markdown::MarkdownParserAdapter::new());
        registry.register(pdf::PdfParserAdapter::new(llm_client));
        registry
    }

    /// List all registered parser names.
    pub fn parser_names(&self) -> Vec<&str> {
        self.parsers.keys().map(|s| s.as_str()).collect()
    }

    /// List all supported file extensions (lowercase, no dot).
    pub fn supported_extensions(&self) -> Vec<&str> {
        self.extension_map.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::default_parsers(None)
    }
}

impl std::fmt::Debug for ParserRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParserRegistry")
            .field("parsers", &self.parsers.keys().collect::<Vec<_>>())
            .field("extensions", &self.extension_map)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Legacy free functions (backward compat — delegate to default registry)
// ---------------------------------------------------------------------------

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
        DocumentFormat::Pdf => Err(vectorless_error::Error::Parse(
            "PDF requires bytes, not string content".to_string(),
        )),
        _ => Err(vectorless_error::Error::Parse(
            format!("Unsupported format for content parsing: {:?}", format),
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
        _ => Err(vectorless_error::Error::Parse(
            format!("Unsupported format for file parsing: {:?}", format),
        )),
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
            let content = std::str::from_utf8(bytes).map_err(|e| {
                vectorless_error::Error::Parse(format!("Invalid UTF-8 content: {}", e))
            })?;
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
        _ => Err(vectorless_error::Error::Parse(
            format!("Unsupported format for bytes parsing: {:?}", format),
        )),
    }
}

/// Detect document format from a file extension.
pub fn format_from_extension(ext: &str) -> Option<DocumentFormat> {
    DocumentFormat::from_extension(ext)
}
