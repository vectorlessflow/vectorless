// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Parser registry for managing document parsers.
//!
//! This module provides a registry for document parsers, allowing
//! dynamic registration and retrieval of parsers by format.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::core::{DocumentParser, Result, Error};
use crate::document::{DocumentFormat, MarkdownParser};

/// Registry for document parsers.
///
/// Parsers can be registered by format and retrieved at runtime.
pub struct ParserRegistry {
    /// Registered parsers by format.
    parsers: Arc<RwLock<HashMap<DocumentFormat, Box<dyn DocumentParser>>>>,
}

impl std::fmt::Debug for ParserRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parsers = self.parsers.read().unwrap();
        let formats: Vec<_> = parsers.keys().collect();
        f.debug_struct("ParserRegistry")
            .field("formats", &formats)
            .finish()
    }
}

impl ParserRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            parsers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a registry with default parsers.
    pub fn with_defaults() -> Self {
        let registry = Self::new();
        registry.register_defaults();
        registry
    }

    /// Register default parsers.
    pub fn register_defaults(&self) {
        self.register(Box::new(MarkdownParser::new()));
    }

    /// Register a parser.
    pub fn register(&self, parser: Box<dyn DocumentParser>) {
        let format = parser.format();
        let mut parsers = self.parsers.write().unwrap();
        parsers.insert(format, parser);
    }

    /// Get a parser by format.
    pub fn get(&self, format: DocumentFormat) -> Option<Box<dyn DocumentParser>> {
        let parsers = self.parsers.read().unwrap();
        // Clone is not possible for trait objects, so we return a reference
        // For now, we'll create new instances for known formats
        match format {
            DocumentFormat::Markdown => Some(Box::new(MarkdownParser::new())),
            _ => None,
        }
    }

    /// Check if a format is supported.
    pub fn supports(&self, format: DocumentFormat) -> bool {
        let parsers = self.parsers.read().unwrap();
        parsers.contains_key(&format) || matches!(format, DocumentFormat::Markdown)
    }

    /// List supported formats.
    pub fn supported_formats(&self) -> Vec<DocumentFormat> {
        let parsers = self.parsers.read().unwrap();
        let mut formats: Vec<DocumentFormat> = parsers.keys().copied().collect();
        // Ensure Markdown is always included
        if !formats.contains(&DocumentFormat::Markdown) {
            formats.push(DocumentFormat::Markdown);
        }
        formats
    }

    /// Parse content using the appropriate parser.
    pub async fn parse(&self, content: &str, format: DocumentFormat) -> Result<crate::document::ParseResult> {
        let parser = self.get(format)
            .ok_or_else(|| Error::Parse(format!("Unsupported format: {:?}", format)))?;
        parser.parse(content).await
    }

    /// Parse a file using the appropriate parser.
    pub async fn parse_file(&self, path: &std::path::Path) -> Result<crate::document::ParseResult> {
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| Error::Parse("Could not determine file extension".to_string()))?;

        let format = DocumentFormat::from_extension(ext)
            .ok_or_else(|| Error::Parse(format!("Unknown format: {}", ext)))?;

        self.parse_file_as(path, format).await
    }

    /// Parse a file with a specific format.
    pub async fn parse_file_as(
        &self,
        path: &std::path::Path,
        format: DocumentFormat,
    ) -> Result<crate::document::ParseResult> {
        let parser = self.get(format)
            .ok_or_else(|| Error::Parse(format!("Unsupported format: {:?}", format)))?;
        parser.parse_file(path).await
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
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
        let registry = ParserRegistry::new();
        let formats = registry.supported_formats();
        assert!(formats.contains(&DocumentFormat::Markdown));
    }
}
