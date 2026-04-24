// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Compile input for document compilation operations.
//!
//! [`CompileInput`] supports single or multiple document sources:
//! - **File path** — Load and parse a file from disk
//! - **Content string** — Parse content directly (HTML, Markdown, text)
//! - **Byte data** — Parse binary data (PDF, DOCX)

use std::path::PathBuf;

use vectorless_document::DocumentFormat;

use super::types::{CompileMode, CompileOptions};

// ============================================================
// Compile Source
// ============================================================

/// The source of document content for compilation.
#[derive(Debug, Clone)]
pub(crate) enum CompileSource {
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
// Compile Input
// ============================================================

/// Input for document compilation operations.
///
/// Supports single or multiple document sources. When multiple sources
/// are provided, each is compiled independently and the results are
/// collected into [`CompileOutput`](super::CompileOutput).
#[derive(Debug, Clone)]
pub struct CompileInput {
    /// Document sources (supports multiple).
    pub(crate) sources: Vec<CompileSource>,

    /// Optional document name for metadata (single-source only).
    pub(crate) name: Option<String>,

    /// Indexing options.
    pub(crate) options: CompileOptions,
}

impl CompileInput {
    /// Create from a single file path.
    ///
    /// The document format is automatically detected from the file extension.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            sources: vec![CompileSource::Path(path.into())],
            name: None,
            options: CompileOptions::default(),
        }
    }

    /// Create from multiple file paths.
    pub fn from_paths(paths: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        Self {
            sources: paths
                .into_iter()
                .map(|p| CompileSource::Path(p.into()))
                .collect(),
            name: None,
            options: CompileOptions::default(),
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
            options: CompileOptions::default(),
        }
    }

    /// Recursively or non-recursively collect supported files.
    fn collect_files(
        dir: &std::path::Path,
        extensions: &[&str],
        recursive: bool,
        sources: &mut Vec<CompileSource>,
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
                        sources.push(CompileSource::Path(path));
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
            sources: vec![CompileSource::Content {
                data: content.into(),
                format,
            }],
            name: None,
            options: CompileOptions::default(),
        }
    }

    /// Create from binary data.
    pub fn from_bytes(bytes: Vec<u8>, format: DocumentFormat) -> Self {
        Self {
            sources: vec![CompileSource::Bytes {
                data: bytes,
                format,
            }],
            name: None,
            options: CompileOptions::default(),
        }
    }

    /// Set the document name (single-source only).
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the indexing options.
    pub fn with_options(mut self, options: CompileOptions) -> Self {
        self.options = options;
        self
    }

    /// Set the indexing mode.
    pub fn with_mode(mut self, mode: CompileMode) -> Self {
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
    pub fn options(&self) -> &CompileOptions {
        &self.options
    }
}

impl From<PathBuf> for CompileInput {
    fn from(path: PathBuf) -> Self {
        Self::from_path(path)
    }
}

impl From<&std::path::Path> for CompileInput {
    fn from(path: &std::path::Path) -> Self {
        Self::from_path(path.to_path_buf())
    }
}

impl From<&str> for CompileInput {
    fn from(path: &str) -> Self {
        Self::from_path(path)
    }
}

impl From<String> for CompileInput {
    fn from(path: String) -> Self {
        Self::from_path(path)
    }
}

impl std::fmt::Display for CompileSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileSource::Path(p) => write!(f, "path:{}", p.display()),
            CompileSource::Content { format, .. } => write!(f, "content:{}", format.extension()),
            CompileSource::Bytes { format, .. } => write!(f, "bytes:{}", format.extension()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_path() {
        let ctx = CompileInput::from_path("./test.md");
        assert_eq!(ctx.len(), 1);
        assert!(ctx.name.is_none());
    }

    #[test]
    fn test_from_paths() {
        let ctx = CompileInput::from_paths(vec!["./a.md", "./b.pdf"]);
        assert_eq!(ctx.len(), 2);
    }

    #[test]
    fn test_from_content() {
        let ctx = CompileInput::from_content("# Title", DocumentFormat::Markdown);
        assert_eq!(ctx.len(), 1);
    }

    #[test]
    fn test_from_bytes() {
        let ctx = CompileInput::from_bytes(vec![1, 2, 3], DocumentFormat::Pdf);
        assert_eq!(ctx.len(), 1);
    }

    #[test]
    fn test_with_name() {
        let ctx = CompileInput::from_path("./test.md").with_name("My Document");
        assert_eq!(ctx.name(), Some("My Document"));
    }

    #[test]
    fn test_with_mode() {
        let ctx = CompileInput::from_path("./test.md").with_mode(CompileMode::Force);
        assert_eq!(ctx.options.mode, CompileMode::Force);
    }

    #[test]
    fn test_from_path_trait() {
        let ctx = CompileInput::from(PathBuf::from("./test.md"));
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
        let ctx = CompileInput::from_dir(&tmp, false);
        assert_eq!(ctx.len(), 1); // only a.md

        // Recursive: all levels
        let ctx = CompileInput::from_dir(&tmp, true);
        assert_eq!(ctx.len(), 3); // a.md, b.md, c.pdf

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
