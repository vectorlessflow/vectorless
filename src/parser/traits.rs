// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Parser trait definition.

use async_trait::async_trait;
use std::path::Path;

use super::{DocumentFormat, ParseResult};
use crate::error::Result;

/// A parser for extracting content from documents.
///
/// Implementations parse different document formats and produce
/// a sequence of raw nodes that can be organized into a tree.
///
/// # Example
///
/// ```rust
/// use vectorless::parser::{DocumentParser, MarkdownParser};
///
/// # #[tokio::main]
/// # async fn main() -> vectorless::domain::Result<()> {
/// let parser = MarkdownParser::new();
/// let content = "# Title\n\nContent here.";
/// let result = parser.parse(content).await?;
/// println!("Found {} nodes", result.node_count());
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait DocumentParser: Send + Sync {
    /// Get the document format this parser handles.
    fn format(&self) -> DocumentFormat;

    /// Parse content from a string.
    ///
    /// # Arguments
    ///
    /// * `content` - The document content as a string
    ///
    /// # Returns
    ///
    /// A [`ParseResult`] containing extracted nodes and metadata.
    async fn parse(&self, content: &str) -> Result<ParseResult>;

    /// Parse content from a file.
    ///
    /// Default implementation reads the file and calls [`parse`](Self::parse).
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file
    async fn parse_file(&self, path: &Path) -> Result<ParseResult> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| crate::Error::Parse(format!("Failed to read file: {}", e)))?;

        self.parse(&content).await
    }
}
