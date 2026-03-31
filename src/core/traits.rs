// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Core traits for the vectorless library.
//!
//! This module defines the main extension points of the library:
//! - [`DocumentParser`] - Parse documents into raw nodes
//! - [`Summarizer`] - Generate summaries for tree nodes

use async_trait::async_trait;
use std::path::Path;

use super::{DocumentTree, NodeId, Result};

// ============================================================
// Document Parser Trait
// ============================================================

/// A parser for extracting content from documents.
///
/// Implementations parse different document formats and produce
/// a sequence of raw nodes that can be organized into a tree.
///
/// # Example
///
/// ```rust
/// use vectorless::core::DocumentParser;
/// use vectorless::document::MarkdownParser;
/// use async_trait::async_trait;
///
/// # #[tokio::main]
/// # async fn main() -> vectorless::core::Result<()> {
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
    fn format(&self) -> crate::document::DocumentFormat;

    /// Parse content from a string.
    ///
    /// # Arguments
    ///
    /// * `content` - The document content as a string
    ///
    /// # Returns
    ///
    /// A [`ParseResult`] containing extracted nodes and metadata.
    async fn parse(&self, content: &str) -> Result<crate::document::ParseResult>;

    /// Parse content from a file.
    ///
    /// Default implementation reads the file and calls [`parse`](Self::parse).
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file
    async fn parse_file(&self, path: &Path) -> Result<crate::document::ParseResult> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| crate::core::Error::Parse(format!("Failed to read file: {}", e)))?;

        self.parse(&content).await
    }
}

// ============================================================
// Summarizer Trait
// ============================================================

/// A summarizer generates concise summaries for tree nodes.
///
/// Implementations can use different strategies:
/// - LLM-based summarization
/// - Extractive summarization
/// - Hybrid approaches
///
/// # Example
///
/// ```rust
/// use vectorless::core::{Summarizer, DocumentTree, NodeId, Result};
/// use async_trait::async_trait;
///
/// struct MySummarizer;
///
/// #[async_trait]
/// impl Summarizer for MySummarizer {
///     async fn summarize(&self, tree: &DocumentTree, node: NodeId) -> Result<String> {
///         let content = tree.get(node)
///             .map(|n| n.content.as_str())
///             .unwrap_or("");
///         Ok(format!("Summary: {}", &content[..50.min(content.len())]))
///     }
/// }
/// ```
#[async_trait]
pub trait Summarizer: Send + Sync {
    /// Generate a summary for the given node.
    ///
    /// # Arguments
    ///
    /// * `tree` - The document tree containing the node
    /// * `node` - The node to summarize
    ///
    /// # Returns
    ///
    /// A summary string, or an error if summarization fails.
    async fn summarize(&self, tree: &DocumentTree, node: NodeId) -> Result<String>;
}

// ============================================================
// Configuration Types
// ============================================================

/// Configuration for summarization behavior.
#[derive(Debug, Clone)]
pub struct SummarizerConfig {
    /// Maximum tokens for the summary.
    pub max_tokens: usize,

    /// Whether to include child content in summaries.
    pub include_children: bool,

    /// Minimum content length to trigger summarization.
    pub min_content_length: usize,
}

impl Default for SummarizerConfig {
    fn default() -> Self {
        Self {
            max_tokens: 200,
            include_children: false,
            min_content_length: 100,
        }
    }
}
