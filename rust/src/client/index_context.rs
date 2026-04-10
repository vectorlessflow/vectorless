// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index context for document indexing operations.
//!
//! This module provides [`IndexContext`], a unified type for specifying
//! document input sources for the [`Engine::index`](super::Engine::index) method.
//!
//! # Overview
//!
//! `IndexContext` supports three input types:
//! - **File path** - Load and parse a file from disk
//! - **Content string** - Parse content directly (for HTML, Markdown, text)
//! - **Byte data** - Parse binary data (for PDF, DOCX)
//!
//! # Examples
//!
//! ## From file path
//!
//! ```rust,no_run
//! use vectorless::client::IndexContext;
//!
//! let ctx = IndexContext::from_path("./document.md");
//! ```
//!
//! ## From content string
//!
//! ```rust
//! use vectorless::client::IndexContext;
//! use vectorless::parser::DocumentFormat;
//!
//! let html = "<html><body><h1>Title</h1><p>Content</p></body></html>";
//! let ctx = IndexContext::from_content(html, DocumentFormat::Html)
//!     .with_name("webpage");
//! ```
//!
//! ## From bytes
//!
//! ```rust
//! use vectorless::client::IndexContext;
//! use vectorless::parser::DocumentFormat;
//!
//! let pdf_bytes = vec![/* PDF binary data */];
//! let ctx = IndexContext::from_bytes(pdf_bytes, DocumentFormat::Pdf);
//! ```
//!
//! ## With options
//!
//! ```rust,no_run
//! use vectorless::client::{IndexContext, IndexMode};
//!
//! let ctx = IndexContext::from_path("./document.pdf")
//!     .with_mode(IndexMode::Force);
//! ```

use std::path::PathBuf;

use crate::parser::DocumentFormat;

use super::types::{IndexMode, IndexOptions};

// ============================================================
// Index Source
// ============================================================

/// The source of document content for indexing.
///
/// This enum represents the different ways a document can be provided
/// to the indexing pipeline.
#[derive(Debug, Clone)]
pub(crate) enum IndexSource {
    /// Load document from a file path.
    ///
    /// The format is detected from the file extension.
    Path(PathBuf),

    /// Parse document from a string.
    ///
    /// Used for text-based formats like HTML and Markdown.
    /// The format must be explicitly specified.
    Content {
        /// The document content as a UTF-8 string.
        data: String,
        /// The document format.
        format: DocumentFormat,
    },

    /// Parse document from binary data.
    ///
    /// Used for binary formats like PDF and DOCX.
    /// The format must be explicitly specified.
    Bytes {
        /// The document content as raw bytes.
        data: Vec<u8>,
        /// The document format.
        format: DocumentFormat,
    },
}

impl IndexSource {
    /// Get the format of this source, if known.
    ///
    /// Returns `None` for `Path` sources (format detected from extension).
    pub fn format(&self) -> Option<DocumentFormat> {
        match self {
            IndexSource::Path(_) => None,
            IndexSource::Content { format, .. } => Some(*format),
            IndexSource::Bytes { format, .. } => Some(*format),
        }
    }

    /// Check if this is a path source.
    pub fn is_path(&self) -> bool {
        matches!(self, IndexSource::Path(_))
    }

    /// Check if this is a content source.
    pub fn is_content(&self) -> bool {
        matches!(self, IndexSource::Content { .. })
    }

    /// Check if this is a bytes source.
    pub fn is_bytes(&self) -> bool {
        matches!(self, IndexSource::Bytes { .. })
    }
}

// ============================================================
// Index Context
// ============================================================

/// Context for document indexing operations.
///
/// `IndexContext` provides a unified interface for specifying document
/// input sources. It supports files, content strings, and binary data.
///
/// # Type Parameters
///
/// The context is constructed using one of:
/// - [`IndexContext::from_path`] - Load from file
/// - [`IndexContext::from_content`] - Parse string content
/// - [`IndexContext::from_bytes`] - Parse binary data
///
/// Additional configuration can be chained:
/// - [`with_name`](IndexContext::with_name) - Set document name
/// - [`with_options`](IndexContext::with_options) - Set indexing options
/// - [`with_mode`](IndexContext::with_mode) - Set indexing mode
///
/// # Examples
///
/// ```rust,no_run
/// use vectorless::client::{EngineBuilder, IndexContext, IndexMode};
/// use vectorless::parser::DocumentFormat;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let engine = EngineBuilder::new()
///     .with_workspace("./data")
///     .build()
///     .await?;
///
/// // Index from file
/// let id1 = engine.index(IndexContext::from_path("./doc.md")).await?;
///
/// // Index HTML content
/// let html = "<h1>Title</h1><p>Content</p>";
/// let id2 = engine.index(
///     IndexContext::from_content(html, DocumentFormat::Html)
///         .with_name("webpage")
/// ).await?;
///
/// // Index with force mode
/// let id3 = engine.index(
///     IndexContext::from_path("./doc.pdf")
///         .with_mode(IndexMode::Force)
/// ).await?;
///
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct IndexContext {
    /// The document source.
    pub(crate) source: IndexSource,

    /// Optional document name for metadata.
    ///
    /// If not set, the name is derived from:
    /// - File name (for path sources)
    /// - "untitled" (for content/bytes sources)
    pub(crate) name: Option<String>,

    /// Indexing options.
    pub(crate) options: IndexOptions,
}

impl IndexContext {
    /// Create an index context from a file path.
    ///
    /// The document format is automatically detected from the file extension.
    ///
    /// # Supported Extensions
    ///
    /// - `.md`, `.markdown` → Markdown
    /// - `.pdf` → PDF
    /// - `.docx` → DOCX
    /// - `.html`, `.htm` → HTML
    /// - `.txt` → Plain text
    ///
    /// # Example
    ///
    /// ```rust
    /// use vectorless::client::IndexContext;
    ///
    /// let ctx = IndexContext::from_path("./documents/report.pdf");
    /// ```
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            source: IndexSource::Path(path.into()),
            name: None,
            options: IndexOptions::default(),
        }
    }

    /// Create an index context from a content string.
    ///
    /// Use this for text-based formats where you have the content
    /// as a string. The format must be explicitly specified.
    ///
    /// # Example
    ///
    /// ```rust
    /// use vectorless::client::IndexContext;
    /// use vectorless::parser::DocumentFormat;
    ///
    /// let markdown = "# Title\n\nContent here.";
    /// let ctx = IndexContext::from_content(markdown, DocumentFormat::Markdown);
    /// ```
    pub fn from_content(content: impl Into<String>, format: DocumentFormat) -> Self {
        Self {
            source: IndexSource::Content {
                data: content.into(),
                format,
            },
            name: None,
            options: IndexOptions::default(),
        }
    }

    /// Create an index context from binary data.
    ///
    /// Use this for binary formats like PDF and DOCX where you
    /// have the raw bytes. The format must be explicitly specified.
    ///
    /// # Example
    ///
    /// ```rust
    /// use vectorless::client::IndexContext;
    /// use vectorless::parser::DocumentFormat;
    ///
    /// let pdf_bytes: Vec<u8> = vec![/* PDF binary data */];
    /// let ctx = IndexContext::from_bytes(pdf_bytes, DocumentFormat::Pdf);
    /// ```
    pub fn from_bytes(bytes: Vec<u8>, format: DocumentFormat) -> Self {
        Self {
            source: IndexSource::Bytes {
                data: bytes,
                format,
            },
            name: None,
            options: IndexOptions::default(),
        }
    }

    /// Set the document name.
    ///
    /// The name is used in document metadata and listings.
    /// If not set, it's derived from the source.
    ///
    /// # Example
    ///
    /// ```rust
    /// use vectorless::client::IndexContext;
    /// use vectorless::parser::DocumentFormat;
    ///
    /// let ctx = IndexContext::from_content("<html>...</html>", DocumentFormat::Html)
    ///     .with_name("homepage");
    /// ```
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the indexing options.
    ///
    /// # Example
    ///
    /// ```rust
    /// use vectorless::client::{IndexContext, IndexOptions, IndexMode};
    ///
    /// let options = IndexOptions {
    ///     mode: IndexMode::Force,
    ///     ..Default::default()
    /// };
    ///
    /// let ctx = IndexContext::from_path("./doc.md")
    ///     .with_options(options);
    /// ```
    pub fn with_options(mut self, options: IndexOptions) -> Self {
        self.options = options;
        self
    }

    /// Set the indexing mode.
    ///
    /// This is a convenience method for setting just the mode.
    ///
    /// # Modes
    ///
    /// - [`IndexMode::Default`] - Skip if already indexed (default)
    /// - [`IndexMode::Force`] - Always re-index
    /// - [`IndexMode::Incremental`] - Only re-index changed files
    ///
    /// # Example
    ///
    /// ```rust
    /// use vectorless::client::{IndexContext, IndexMode};
    ///
    /// let ctx = IndexContext::from_path("./doc.md")
    ///     .with_mode(IndexMode::Force);
    /// ```
    pub fn with_mode(mut self, mode: IndexMode) -> Self {
        self.options.mode = mode;
        self
    }

    /// Get the document name, if set.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Get the indexing options.
    pub fn options(&self) -> &IndexOptions {
        &self.options
    }
}

impl From<PathBuf> for IndexContext {
    fn from(path: PathBuf) -> Self {
        Self::from_path(path)
    }
}

impl From<&std::path::Path> for IndexContext {
    fn from(path: &std::path::Path) -> Self {
        Self::from_path(path.to_path_buf())
    }
}

impl From<&str> for IndexContext {
    fn from(path: &str) -> Self {
        Self::from_path(path)
    }
}

impl From<String> for IndexContext {
    fn from(path: String) -> Self {
        Self::from_path(path)
    }
}

impl std::fmt::Display for IndexSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexSource::Path(p) => write!(f, "path:{}", p.display()),
            IndexSource::Content { format, .. } => write!(f, "content:{}", format.extension()),
            IndexSource::Bytes { format, .. } => write!(f, "bytes:{}", format.extension()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_path() {
        let ctx = IndexContext::from_path("./test.md");
        assert!(ctx.source.is_path());
        assert!(ctx.name.is_none());
    }

    #[test]
    fn test_from_content() {
        let ctx = IndexContext::from_content("# Title", DocumentFormat::Markdown);
        assert!(ctx.source.is_content());
        assert!(ctx.name.is_none());
    }

    #[test]
    fn test_from_bytes() {
        let ctx = IndexContext::from_bytes(vec![1, 2, 3], DocumentFormat::Pdf);
        assert!(ctx.source.is_bytes());
    }

    #[test]
    fn test_with_name() {
        let ctx = IndexContext::from_path("./test.md").with_name("My Document");

        assert_eq!(ctx.name(), Some("My Document"));
    }

    #[test]
    fn test_with_mode() {
        let ctx = IndexContext::from_path("./test.md").with_mode(IndexMode::Force);

        assert_eq!(ctx.options.mode, IndexMode::Force);
    }

    #[test]
    fn test_chaining() {
        let ctx = IndexContext::from_content("<html>", DocumentFormat::Html)
            .with_name("page")
            .with_mode(IndexMode::Force);

        assert!(ctx.source.is_content());
        assert_eq!(ctx.name(), Some("page"));
        assert_eq!(ctx.options.mode, IndexMode::Force);
    }

    #[test]
    fn test_from_path_trait() {
        let ctx = IndexContext::from(PathBuf::from("./test.md"));
        assert!(ctx.source.is_path());
    }

    #[test]
    fn test_source_format() {
        let content_source = IndexSource::Content {
            data: "test".to_string(),
            format: DocumentFormat::Html,
        };
        assert_eq!(content_source.format(), Some(DocumentFormat::Html));

        let path_source = IndexSource::Path(PathBuf::from("./test.md"));
        assert_eq!(path_source.format(), None);
    }
}
