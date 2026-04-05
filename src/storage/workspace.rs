// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Workspace management for document collections.
//!
//! A workspace is a directory containing indexed documents and metadata.
//! Uses lazy-loading pattern with LRU cache:
//! - Metadata index always in memory
//! - Full documents loaded on demand with LRU eviction
//!
//! # Structure
//!
//! ```text
//! workspace/
//! ├── _meta.json           # Lightweight index: all document metadata
//! ├── {doc_id_1}.json      # Document 1 full data (tree + pages)
//! ├── {doc_id_2}.json      # Document 2 full data
//! └── ...
//! ```
//!
//! # Thread Safety
//!
//! The workspace uses interior mutability for the LRU cache:
//! - Read operations (`get_meta`, `contains`, `list_documents`) only need `&self`
//! - Cache updates happen internally via `Mutex`
//!
//! # File Locking
//!
//! When enabled (default), the workspace uses an exclusive file lock
//! to prevent concurrent access from multiple processes.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::cache::DocumentCache;
use super::lock::FileLock;
use super::persistence::{PersistedDocument, load_document, save_document};
use crate::error::Result;
use crate::Error;

const META_FILE: &str = "_meta.json";
const LOCK_FILE: &str = ".workspace.lock";
const DEFAULT_CACHE_SIZE: usize = 100;

/// Lightweight metadata entry for the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMetaEntry {
    /// Document ID.
    pub id: String,
    /// Document name/title.
    pub doc_name: String,
    /// Document description.
    #[serde(default)]
    pub doc_description: Option<String>,
    /// Document type (pdf, md, etc.).
    pub doc_type: String,
    /// Source file path.
    #[serde(default)]
    pub path: Option<String>,
    /// Page count (for PDFs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<usize>,
    /// Line count (for markdown).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_count: Option<usize>,
}

/// A workspace for managing indexed documents.
///
/// Uses LRU cache for loaded documents to balance memory usage
/// and access performance. The cache uses interior mutability,
/// so read operations only require `&self`.
#[derive(Debug)]
pub struct Workspace {
    /// Root directory for the workspace.
    root: PathBuf,

    /// Document metadata index (id -> meta).
    /// This is always loaded in memory.
    meta_index: HashMap<String, DocumentMetaEntry>,

    /// LRU cache for loaded documents.
    cache: DocumentCache,

    /// File lock for multi-process safety.
    _lock: Option<FileLock>,
}

/// Options for workspace creation.
#[derive(Debug, Clone)]
pub struct WorkspaceOptions {
    /// Enable file locking (default: true).
    pub file_lock: bool,
    /// LRU cache size (default: 100).
    pub cache_size: usize,
}

impl Default for WorkspaceOptions {
    fn default() -> Self {
        Self {
            file_lock: true,
            cache_size: DEFAULT_CACHE_SIZE,
        }
    }
}

impl Workspace {
    /// Create a new workspace at the given path with default cache size.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        Self::with_options(path, WorkspaceOptions::default())
    }

    /// Create a new workspace with custom LRU cache size.
    pub fn with_cache_size(path: impl Into<PathBuf>, cache_size: usize) -> Result<Self> {
        Self::with_options(path, WorkspaceOptions {
            cache_size,
            ..Default::default()
        })
    }

    /// Create a new workspace with custom options.
    pub fn with_options(path: impl Into<PathBuf>, options: WorkspaceOptions) -> Result<Self> {
        let root = path.into();
        fs::create_dir_all(&root).map_err(Error::Io)?;

        // Acquire file lock if enabled
        let lock = if options.file_lock {
            let lock_path = root.join(LOCK_FILE);
            Some(FileLock::try_lock(&lock_path, true)?)
        } else {
            None
        };

        let mut workspace = Self {
            root,
            meta_index: HashMap::new(),
            cache: DocumentCache::with_capacity(options.cache_size),
            _lock: lock,
        };

        workspace.load_meta_index()?;
        Ok(workspace)
    }

    /// Open an existing workspace, or create if it doesn't exist.
    pub fn open(path: impl Into<PathBuf> + Clone) -> Result<Self> {
        Self::open_with_options(path, WorkspaceOptions::default())
    }

    /// Open with custom cache size.
    pub fn open_with_cache_size(
        path: impl Into<PathBuf> + Clone,
        cache_size: usize,
    ) -> Result<Self> {
        Self::open_with_options(path, WorkspaceOptions {
            cache_size,
            ..Default::default()
        })
    }

    /// Open with custom options.
    pub fn open_with_options(
        path: impl Into<PathBuf> + Clone,
        options: WorkspaceOptions,
    ) -> Result<Self> {
        let root = path.clone().into();
        if root.exists() {
            // Acquire file lock if enabled
            let lock = if options.file_lock {
                let lock_path = root.join(LOCK_FILE);
                Some(FileLock::try_lock(&lock_path, true)?)
            } else {
                None
            };

            let mut workspace = Self {
                root,
                meta_index: HashMap::new(),
                cache: DocumentCache::with_capacity(options.cache_size),
                _lock: lock,
            };
            workspace.load_meta_index()?;
            Ok(workspace)
        } else {
            Self::with_options(path, options)
        }
    }

    /// Get the workspace root path.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// List all document IDs in the workspace.
    pub fn list_documents(&self) -> Vec<&str> {
        self.meta_index.keys().map(|s| s.as_str()).collect()
    }

    /// Get metadata for a document.
    pub fn get_meta(&self, id: &str) -> Option<&DocumentMetaEntry> {
        self.meta_index.get(id)
    }

    /// Check if a document exists.
    pub fn contains(&self, id: &str) -> bool {
        self.meta_index.contains_key(id)
    }

    /// Add a document to the workspace.
    ///
    /// This saves the full document to disk and updates the meta index.
    /// The document is NOT cached (lazy loading on first access).
    pub fn add(&mut self, doc: &PersistedDocument) -> Result<()> {
        let doc_id = doc.meta.id.clone();
        let doc_path = self.document_path(&doc_id);

        // Save full document to disk
        save_document(&doc_path, doc)?;

        // Update meta index (lightweight)
        let meta_entry = DocumentMetaEntry {
            id: doc_id.clone(),
            doc_name: doc.meta.name.clone(),
            doc_description: doc.meta.description.clone(),
            doc_type: doc.meta.format.clone(),
            path: doc
                .meta
                .source_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            page_count: if doc.pages.is_empty() { None } else { Some(doc.pages.len()) },
            line_count: doc.meta.line_count,
        };

        self.meta_index.insert(doc_id.clone(), meta_entry);
        self.save_meta_index()?;

        // Remove from cache if present (will lazy load on next access)
        let _ = self.cache.remove(&doc_id);

        info!("Saved document {} to workspace", doc_id);
        Ok(())
    }

    /// Load a document from the workspace.
    ///
    /// Uses LRU cache: returns cached version if available,
    /// otherwise loads from disk and caches it.
    ///
    /// This method only requires `&self` (interior mutability for cache).
    pub fn load(&self, id: &str) -> Result<Option<PersistedDocument>> {
        if !self.contains(id) {
            return Ok(None);
        }

        // Check LRU cache first
        if let Some(cached) = self.cache.get(id)? {
            debug!("Cache hit for document {}", id);
            return Ok(Some(cached));
        }

        // Load from disk
        let doc_path = self.document_path(id);
        if !doc_path.exists() {
            warn!("Document {} in meta index but file missing", id);
            return Ok(None);
        }

        let doc = load_document(&doc_path)?;

        // Add to LRU cache
        self.cache.put(id.to_string(), doc.clone())?;

        debug!("Loaded document {} from disk (cached)", id);
        Ok(Some(doc))
    }

    /// Remove a document from the workspace.
    pub fn remove(&mut self, id: &str) -> Result<bool> {
        if !self.contains(id) {
            return Ok(false);
        }

        let doc_path = self.document_path(id);
        if doc_path.exists() {
            fs::remove_file(&doc_path).map_err(Error::Io)?;
        }

        self.meta_index.remove(id);

        // Remove from cache
        let _ = self.cache.remove(id);

        self.save_meta_index()?;

        info!("Removed document {} from workspace", id);
        Ok(true)
    }

    /// Get the number of documents in the workspace.
    pub fn len(&self) -> usize {
        self.meta_index.len()
    }

    /// Check if the workspace is empty.
    pub fn is_empty(&self) -> bool {
        self.meta_index.is_empty()
    }

    /// Get the number of items currently in the LRU cache.
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// Get cache utilization (0.0 to 1.0).
    pub fn cache_utilization(&self) -> f64 {
        self.cache.utilization()
    }

    /// Clear the LRU cache (does not remove documents from workspace).
    pub fn clear_cache(&self) -> Result<()> {
        self.cache.clear()?;
        debug!("Cleared document cache");
        Ok(())
    }

    /// Get the path for a document file.
    fn document_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{}.json", id))
    }

    /// Get the path for the meta index file.
    fn meta_path(&self) -> PathBuf {
        self.root.join(META_FILE)
    }

    /// Load the meta index from disk.
    fn load_meta_index(&mut self) -> Result<()> {
        let meta_path = self.meta_path();

        if !meta_path.exists() {
            // Try to rebuild from existing files
            self.rebuild_meta_index()?;
            return Ok(());
        }

        let content = fs::read_to_string(&meta_path).map_err(Error::Io)?;

        let meta: HashMap<String, DocumentMetaEntry> = serde_json::from_str(&content)
            .map_err(|e| Error::Parse(format!("Failed to parse meta index: {}", e)))?;

        self.meta_index = meta;
        info!(
            "Loaded {} document(s) from workspace index",
            self.meta_index.len()
        );
        Ok(())
    }

    /// Save the meta index to disk.
    fn save_meta_index(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.meta_index)
            .map_err(|e| Error::Parse(format!("Failed to serialize meta index: {}", e)))?;

        fs::write(self.meta_path(), content).map_err(Error::Io)?;

        Ok(())
    }

    /// Rebuild the meta index from existing document files.
    fn rebuild_meta_index(&mut self) -> Result<()> {
        let entries: Vec<_> = fs::read_dir(&self.root)
            .map_err(Error::Io)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|ext| ext == "json")
                    .unwrap_or(false)
            })
            .filter_map(|entry| {
                let path = entry.path();
                // Skip the meta file itself
                if path.file_stem()?.to_str()? == "_meta" {
                    return None;
                }
                // Try to load the document and extract metadata
                load_document(&path).ok().map(|doc| {
                    let doc_id = doc.meta.id.clone();
                    let meta_entry = DocumentMetaEntry {
                        id: doc_id.clone(),
                        doc_name: doc.meta.name,
                        doc_description: doc.meta.description,
                        doc_type: doc.meta.format,
                        path: doc
                            .meta
                            .source_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string()),
                        page_count: if doc.pages.is_empty() { None } else { Some(doc.pages.len()) },
                        line_count: doc.meta.line_count,
                    };
                    (doc_id, meta_entry)
                })
            })
            .collect();

        for (id, entry) in entries {
            self.meta_index.insert(id, entry);
        }

        if !self.meta_index.is_empty() {
            self.save_meta_index()?;
            info!(
                "Rebuilt index from {} document file(s)",
                self.meta_index.len()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_workspace_create() {
        let temp = TempDir::new().unwrap();
        let workspace = Workspace::new(temp.path()).unwrap();

        assert!(workspace.is_empty());
        assert_eq!(workspace.len(), 0);
    }

    #[test]
    fn test_workspace_open() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("workspace");

        // Use options without file lock to allow reopening
        let options = WorkspaceOptions {
            file_lock: false,
            ..Default::default()
        };

        // Create new
        let workspace = Workspace::open_with_options(&path, options.clone()).unwrap();
        assert!(workspace.is_empty());

        // Reopen existing (need to drop first workspace to release lock)
        drop(workspace);
        let workspace2 = Workspace::open_with_options(&path, options).unwrap();
        assert!(workspace2.is_empty());
    }

    #[test]
    fn test_workspace_cache_operations() {
        let temp = TempDir::new().unwrap();
        let workspace = Workspace::with_cache_size(temp.path(), 5).unwrap();

        assert_eq!(workspace.cache_len(), 0);
        assert_eq!(workspace.cache.utilization(), 0.0);

        workspace.clear_cache().unwrap();
        assert_eq!(workspace.cache_len(), 0);
    }
}
