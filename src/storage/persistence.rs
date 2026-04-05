// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Persistence utilities for saving and loading document indices.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

use crate::{DocumentTree, Error};
use crate::error::Result;

/// Metadata for a persisted document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    /// Unique document identifier.
    pub id: String,

    /// Document name/title.
    pub name: String,

    /// Document format (md, pdf, etc.).
    pub format: String,

    /// Source file path.
    pub source_path: Option<PathBuf>,

    /// Document description.
    pub description: Option<String>,

    /// Page count (for PDFs).
    pub page_count: Option<usize>,

    /// Line count (for text files).
    pub line_count: Option<usize>,

    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Last modified timestamp.
    pub modified_at: chrono::DateTime<chrono::Utc>,
}

impl DocumentMeta {
    /// Create new document metadata.
    pub fn new(id: impl Into<String>, name: impl Into<String>, format: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            format: format.into(),
            source_path: None,
            description: None,
            page_count: None,
            line_count: None,
            created_at: now,
            modified_at: now,
        }
    }

    /// Set the source path.
    pub fn with_source_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    /// Set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// A persisted document index containing tree and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedDocument {
    /// Document metadata.
    pub meta: DocumentMeta,

    /// The document tree structure.
    pub tree: DocumentTree,

    /// Per-page content (for PDFs).
    #[serde(default)]
    pub pages: Vec<PageContent>,
}

impl PersistedDocument {
    /// Create a new persisted document.
    pub fn new(meta: DocumentMeta, tree: DocumentTree) -> Self {
        Self {
            meta,
            tree,
            pages: Vec::new(),
        }
    }

    /// Add page content.
    pub fn add_page(&mut self, page: usize, content: impl Into<String>) {
        self.pages.push(PageContent {
            page,
            content: content.into(),
        });
    }
}

/// Content for a single page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageContent {
    /// Page number (1-based).
    pub page: usize,

    /// Page text content.
    pub content: String,
}

/// Save a document to a JSON file.
pub fn save_document(path: &Path, doc: &PersistedDocument) -> Result<()> {
    let json = serde_json::to_string_pretty(doc)
        .map_err(|e| Error::Io(io::Error::new(io::ErrorKind::Other, e)))?;

    std::fs::write(path, json).map_err(|e| Error::Io(e))?;

    Ok(())
}

/// Load a document from a JSON file.
pub fn load_document(path: &Path) -> Result<PersistedDocument> {
    let json = std::fs::read_to_string(path).map_err(|e| Error::Io(e))?;

    let doc: PersistedDocument = serde_json::from_str(&json)
        .map_err(|e| Error::Parse(format!("Failed to parse document: {}", e)))?;

    Ok(doc)
}

/// Save the workspace index (metadata for all documents).
pub fn save_index(path: &Path, entries: &[DocumentMeta]) -> Result<()> {
    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| Error::Io(io::Error::new(io::ErrorKind::Other, e)))?;

    std::fs::write(path, json).map_err(|e| Error::Io(e))?;

    Ok(())
}

/// Load the workspace index.
pub fn load_index(path: &Path) -> Result<Vec<DocumentMeta>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let json = std::fs::read_to_string(path).map_err(|e| Error::Io(e))?;

    let entries: Vec<DocumentMeta> = serde_json::from_str(&json)
        .map_err(|e| Error::Parse(format!("Failed to parse index: {}", e)))?;

    Ok(entries)
}
