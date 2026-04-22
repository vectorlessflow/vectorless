// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index context for document indexing operations.
//!
//! [`IndexContext`] supports single or multiple document sources:
//! - **File path** — Load and parse a file from disk
//! - **Content string** — Parse content directly (HTML, Markdown, text)
//! - **Byte data** — Parse binary data (PDF, DOCX)
//!
//! # Single document
//!
//! ```rust,no_run
//! use vectorless::client::IndexContext;
//!
//! let ctx = IndexContext::from_path("./document.md");
//! ```
//!
//! # Multiple documents
//!
//! ```rust,no_run
//! use vectorless::client::IndexContext;
//!
//! let ctx = IndexContext::from_paths(vec!["./doc1.md", "./doc2.pdf"]);
//! ```
//!
//! # From directory
//!
//! ```rust,no_run
//! use vectorless::client::IndexContext;
//!
//! // Non-recursive (top-level only)
//! let ctx = IndexContext::from_dir("./documents", false);
//!
//! // Recursive (includes subdirectories)
//! let ctx = IndexContext::from_dir("./documents", true);
//! ```

use std::path::PathBuf;

use crate::index::parse::DocumentFormat;

use super::types::{IndexMode, IndexOptions};

// ============================================================
// Index Source
// ============================================================

/// The source of document content for indexing.
#[derive(Debug, Clone)]
pub(crate) enum IndexSource {
    /// Load document from a file path.
    Path(PathBuf),

    /// Parse document from a string.
    Content {
        data: String,
        format: DocumentFormat,
    },

    /// Parse document from binary data.
    Bytes {
        data: Vec<u8>,
        format: DocumentFormat,
    },
}

// ============================================================
// Index Context
// ============================================================

/// Context for document indexing operations.
///
/// Supports single or multiple document sources. When multiple sources
/// are provided, each is indexed independently and the results are
/// collected into [`IndexResult`](super::IndexResult).
///
/// # Examples
///
/// ```rust,no_run
/// use vectorless::client::IndexContext;
/// use vectorless::client::DocumentFormat;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let engine = vectorless::EngineBuilder::new().build().await?;
/// // Single file
/// let result = engine.index(IndexContext::from_path("./doc.md")).await?;
///
/// // Multiple files
/// let result = engine.index(
///     IndexContext::from_paths(vec!["./doc1.md", "./doc2.pdf"])
/// ).await?;
///
/// // Entire directory
/// let result = engine.index(IndexContext::from_dir("./docs", false)).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct IndexContext {
    /// Document sources (supports multiple).
    pub(crate) sources: Vec<IndexSource>,

    /// Optional document name for metadata (single-source only).
    pub(crate) name: Option<String>,

    /// Indexing options.
    pub(crate) options: IndexOptions,
}

impl IndexContext {
    /// Create from a single file path.
    ///
    /// The document format is automatically detected from the file extension.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            sources: vec![IndexSource::Path(path.into())],
            name: None,
            options: IndexOptions::default(),
        }
    }

    /// Create from multiple file paths.
    pub fn from_paths(paths: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        Self {
            sources: paths
                .into_iter()
                .map(|p| IndexSource::Path(p.into()))
                .collect(),
            name: None,
            options: IndexOptions::default(),
        }
    }

    /// Create from a directory path.
    ///
    /// Indexes all supported files in the directory.
    /// Supported extensions: `.md`, `.pdf`.
    ///
    /// Set `recursive` to `true` to include subdirectories.
    pub fn from_dir(dir: impl Into<PathBuf>, recursive: bool) -> Self {
        Self::scan_dir(dir, recursive)
    }

    /// Internal: scan a directory for supported document files.
    fn scan_dir(dir: impl Into<PathBuf>, recursive: bool) -> Self {
        let dir = dir.into();
        let supported_extensions = DocumentFormat::SUPPORTED_EXTENSIONS;

        if !dir.exists() {
            tracing::warn!("Directory not found: {}", dir.display());
        }

        let mut sources = Vec::new();
        Self::collect_files(&dir, &supported_extensions, recursive, &mut sources);

        Self {
            sources,
            name: None,
            options: IndexOptions::default(),
        }
    }

    /// Recursively or non-recursively collect supported files.
    fn collect_files(
        dir: &std::path::Path,
        extensions: &[&str],
        recursive: bool,
        sources: &mut Vec<IndexSource>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut subdirs = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if recursive {
                        subdirs.push(path);
                    }
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if extensions.contains(&ext.to_lowercase().as_str()) {
                        sources.push(IndexSource::Path(path));
                    }
                }
            }
            for subdir in subdirs {
                Self::collect_files(&subdir, extensions, recursive, sources);
            }
        }
    }

    /// Create from a content string.
    pub fn from_content(content: impl Into<String>, format: DocumentFormat) -> Self {
        Self {
            sources: vec![IndexSource::Content {
                data: content.into(),
                format,
            }],
            name: None,
            options: IndexOptions::default(),
        }
    }

    /// Create from binary data.
    pub fn from_bytes(bytes: Vec<u8>, format: DocumentFormat) -> Self {
        Self {
            sources: vec![IndexSource::Bytes {
                data: bytes,
                format,
            }],
            name: None,
            options: IndexOptions::default(),
        }
    }

    /// Set the document name (single-source only).
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the indexing options.
    pub fn with_options(mut self, options: IndexOptions) -> Self {
        self.options = options;
        self
    }

    /// Set the indexing mode.
    pub fn with_mode(mut self, mode: IndexMode) -> Self {
        self.options.mode = mode;
        self
    }

    /// Number of document sources.
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Check if there are no sources.
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
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
        assert_eq!(ctx.len(), 1);
        assert!(ctx.name.is_none());
    }

    #[test]
    fn test_from_paths() {
        let ctx = IndexContext::from_paths(vec!["./a.md", "./b.pdf"]);
        assert_eq!(ctx.len(), 2);
    }

    #[test]
    fn test_from_content() {
        let ctx = IndexContext::from_content("# Title", DocumentFormat::Markdown);
        assert_eq!(ctx.len(), 1);
    }

    #[test]
    fn test_from_bytes() {
        let ctx = IndexContext::from_bytes(vec![1, 2, 3], DocumentFormat::Pdf);
        assert_eq!(ctx.len(), 1);
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
    fn test_from_path_trait() {
        let ctx = IndexContext::from(PathBuf::from("./test.md"));
        assert_eq!(ctx.len(), 1);
    }

    #[test]
    fn test_from_dir_with_recursive() {
        // Create a temp directory structure:
        //   tmp/
        //     a.md
        //     sub/
        //       b.md
        //       deep/
        //         c.pdf
        let tmp = std::env::temp_dir().join("vectorless_test_dir_recursive");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("sub/deep")).unwrap();
        std::fs::write(tmp.join("a.md"), "# A").unwrap();
        std::fs::write(tmp.join("sub/b.md"), "# B").unwrap();
        std::fs::write(tmp.join("sub/deep/c.pdf"), b"%PDF").unwrap();
        std::fs::write(tmp.join("sub/deep/ignore.dat"), b"xxx").unwrap();

        // Non-recursive: only top-level
        let ctx = IndexContext::from_dir(&tmp, false);
        assert_eq!(ctx.len(), 1); // only a.md

        // Recursive: all levels
        let ctx = IndexContext::from_dir(&tmp, true);
        assert_eq!(ctx.len(), 3); // a.md, b.md, c.pdf

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
