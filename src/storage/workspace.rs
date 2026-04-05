// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Workspace management for document collections.
//!
//! A workspace manages indexed documents using a storage backend abstraction.
//! Uses lazy-loading pattern with LRU cache:
//! - Metadata index always in memory
//! - Full documents loaded on demand with LRU eviction
//!
//! # Backends
//!
//! The workspace supports different storage backends:
//! - **FileBackend**: File system storage (default)
//! - **MemoryBackend**: In-memory storage (for testing)
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::storage::{Workspace, FileBackend};
//!
//! // Default file-based workspace
//! let mut workspace = Workspace::new("./my_workspace")?;
//!
//! // Or with custom backend
//! let backend = std::sync::Arc::new(FileBackend::new("./my_workspace")?);
//! let mut workspace = Workspace::with_backend(backend)?;
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::backend::{FileBackend, StorageBackend};
use super::cache::DocumentCache;
use super::lock::FileLock;
use super::persistence::{PersistedDocument, load_document_from_bytes, save_document_to_bytes};
use crate::error::Result;
use crate::Error;

const META_KEY: &str = "_meta";
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
/// and access performance.
///
/// # Thread Safety
///
/// The workspace is thread-safe when used with a thread-safe backend.
/// Read operations only require `&self`.
#[derive(Debug)]
pub struct Workspace {
    /// Storage backend.
    backend: Arc<dyn StorageBackend>,
    /// Root path (for file-based backends, used for locking).
    root: Option<PathBuf>,
    /// Document metadata index (id -> meta).
    /// This is always loaded in memory.
    meta_index: HashMap<String, DocumentMetaEntry>,
    /// LRU cache for loaded documents.
    cache: DocumentCache,
    /// File lock for multi-process safety (file backends only).
    _lock: Option<FileLock>,
}

/// Options for workspace creation.
#[derive(Debug, Clone)]
pub struct WorkspaceOptions {
    /// Enable file locking (default: true, only for file backends).
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

impl WorkspaceOptions {
    /// Create new options with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the cache size.
    pub fn with_cache_size(mut self, size: usize) -> Self {
        self.cache_size = size;
        self
    }

    /// Enable or disable file locking.
    pub fn with_file_lock(mut self, enabled: bool) -> Self {
        self.file_lock = enabled;
        self
    }
}

impl Workspace {
    /// Create a new workspace with a storage backend.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let backend = Arc::new(FileBackend::new("./workspace")?);
    /// let workspace = Workspace::with_backend(backend)?;
    /// ```
    pub fn with_backend(backend: Arc<dyn StorageBackend>) -> Result<Self> {
        Self::with_backend_and_options(backend, WorkspaceOptions::default())
    }

    /// Create a workspace with backend and options.
    pub fn with_backend_and_options(
        backend: Arc<dyn StorageBackend>,
        options: WorkspaceOptions,
    ) -> Result<Self> {
        let mut workspace = Self {
            backend,
            root: None,
            meta_index: HashMap::new(),
            cache: DocumentCache::with_capacity(options.cache_size),
            _lock: None,
        };

        workspace.load_meta_index()?;
        Ok(workspace)
    }

    /// Create a new file-based workspace at the given path.
    ///
    /// This is a convenience method that creates a `FileBackend` internally.
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

        // Acquire file lock if enabled
        let lock = if options.file_lock {
            let lock_path = root.join(LOCK_FILE);
            Some(FileLock::try_lock(&lock_path, true)?)
        } else {
            None
        };

        let backend = Arc::new(FileBackend::new(&root)?);

        let mut workspace = Self {
            backend,
            root: Some(root),
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

        // Acquire file lock if enabled
        let lock = if options.file_lock && root.exists() {
            let lock_path = root.join(LOCK_FILE);
            Some(FileLock::try_lock(&lock_path, true)?)
        } else {
            None
        };

        let backend = Arc::new(FileBackend::new(&root)?);

        let mut workspace = Self {
            backend,
            root: Some(root),
            meta_index: HashMap::new(),
            cache: DocumentCache::with_capacity(options.cache_size),
            _lock: lock,
        };

        workspace.load_meta_index()?;
        Ok(workspace)
    }

    /// Get the workspace root path (if file-based).
    pub fn path(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Get the storage backend.
    pub fn backend(&self) -> &dyn StorageBackend {
        self.backend.as_ref()
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
    pub fn add(&mut self, doc: &PersistedDocument) -> Result<()> {
        let doc_id = doc.meta.id.clone();
        let key = self.doc_key(&doc_id);

        // Serialize and save via backend
        let bytes = save_document_to_bytes(doc)?;
        self.backend.put(&key, &bytes)?;

        // Update meta index
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

        // Remove from cache if present
        let _ = self.cache.remove(&doc_id);

        info!("Saved document {} to workspace", doc_id);
        Ok(())
    }

    /// Load a document from the workspace.
    ///
    /// Uses LRU cache: returns cached version if available,
    /// otherwise loads from backend and caches it.
    pub fn load(&self, id: &str) -> Result<Option<PersistedDocument>> {
        if !self.contains(id) {
            return Ok(None);
        }

        // Check LRU cache first
        if let Some(cached) = self.cache.get(id)? {
            debug!("Cache hit for document {}", id);
            return Ok(Some(cached));
        }

        // Load from backend
        let key = self.doc_key(id);
        match self.backend.get(&key)? {
            Some(bytes) => {
                let doc = load_document_from_bytes(&bytes)?;

                // Add to LRU cache
                self.cache.put(id.to_string(), doc.clone())?;

                debug!("Loaded document {} from backend (cached)", id);
                Ok(Some(doc))
            }
            None => {
                warn!("Document {} in meta index but not in backend", id);
                Ok(None)
            }
        }
    }

    /// Remove a document from the workspace.
    pub fn remove(&mut self, id: &str) -> Result<bool> {
        if !self.contains(id) {
            return Ok(false);
        }

        let key = self.doc_key(id);
        self.backend.delete(&key)?;

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

    /// Get cache statistics.
    pub fn cache_stats(&self) -> super::cache::CacheStats {
        self.cache.stats()
    }

    /// Clear the LRU cache (does not remove documents from workspace).
    pub fn clear_cache(&self) -> Result<()> {
        self.cache.clear()?;
        debug!("Cleared document cache");
        Ok(())
    }

    /// Get the storage key for a document.
    fn doc_key(&self, id: &str) -> String {
        format!("doc:{}", id)
    }

    /// Load the meta index from backend.
    fn load_meta_index(&mut self) -> Result<()> {
        match self.backend.get(META_KEY)? {
            Some(bytes) => {
                let meta: HashMap<String, DocumentMetaEntry> = serde_json::from_slice(&bytes)
                    .map_err(|e| Error::Parse(format!("Failed to parse meta index: {}", e)))?;
                self.meta_index = meta;
                info!(
                    "Loaded {} document(s) from workspace index",
                    self.meta_index.len()
                );
            }
            None => {
                // Try to rebuild from existing keys
                self.rebuild_meta_index()?;
            }
        }
        Ok(())
    }

    /// Save the meta index to backend.
    fn save_meta_index(&self) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&self.meta_index)
            .map_err(|e| Error::Parse(format!("Failed to serialize meta index: {}", e)))?;
        self.backend.put(META_KEY, &bytes)?;
        Ok(())
    }

    /// Rebuild the meta index from existing documents.
    fn rebuild_meta_index(&mut self) -> Result<()> {
        let keys = self.backend.keys()?;
        let doc_keys: Vec<_> = keys
            .iter()
            .filter(|k| k.starts_with("doc:"))
            .collect();

        for key in doc_keys {
            if let Some(bytes) = self.backend.get(key)? {
                if let Ok(doc) = load_document_from_bytes(&bytes) {
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
                    self.meta_index.insert(doc_id, meta_entry);
                }
            }
        }

        if !self.meta_index.is_empty() {
            self.save_meta_index()?;
            info!(
                "Rebuilt index from {} document(s)",
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
    fn test_workspace_with_memory_backend() {
        let backend = Arc::new(super::super::backend::MemoryBackend::new());
        let mut workspace = Workspace::with_backend(backend).unwrap();

        assert!(workspace.is_empty());

        // Add a document
        let meta = super::super::persistence::DocumentMeta::new("doc-1", "Test", "md");
        let tree = crate::document::DocumentTree::new("Root", "Content");
        let doc = PersistedDocument::new(meta, tree);

        workspace.add(&doc).unwrap();
        assert_eq!(workspace.len(), 1);

        // Load it back
        let loaded = workspace.load("doc-1").unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().meta.id, "doc-1");
    }

    #[test]
    fn test_workspace_open() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("workspace");

        let options = WorkspaceOptions {
            file_lock: false,
            ..Default::default()
        };

        let workspace = Workspace::open_with_options(&path, options.clone()).unwrap();
        assert!(workspace.is_empty());

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

    #[test]
    fn test_workspace_cache_stats() {
        let backend = Arc::new(super::super::backend::MemoryBackend::new());
        let mut workspace = Workspace::with_backend(backend).unwrap();

        let meta = super::super::persistence::DocumentMeta::new("doc-1", "Test", "md");
        let tree = crate::document::DocumentTree::new("Root", "Content");
        let doc = PersistedDocument::new(meta, tree);
        workspace.add(&doc).unwrap();

        // First load - cache miss
        let _ = workspace.load("doc-1").unwrap();
        let stats = workspace.cache_stats();
        assert_eq!(stats.misses, 1);

        // Second load - cache hit
        let _ = workspace.load("doc-1").unwrap();
        let stats = workspace.cache_stats();
        assert_eq!(stats.hits, 1);
    }
}
