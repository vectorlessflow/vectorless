// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Async workspace management for document collections.
//!
//! This module provides the primary workspace implementation for document
//! persistence, using async I/O for integration with runtimes like Tokio.
//!
//! # Features
//!
//! - **Async I/O** - All operations are async for non-blocking performance
//! - **LRU Cache** - Automatic caching with configurable size
//! - **Thread-Safe** - Fully thread-safe with `Arc<RwLock>`
//! - **Pluggable Backend** - Use file storage, in-memory, or custom backends
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::storage::Workspace;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let workspace = Workspace::new("./workspace").await?;
//!
//!     // Add a document
//!     workspace.add(&doc).await?;
//!
//!     // Load with caching
//!     let loaded = workspace.load_and_cache("doc-1").await?;
//!
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::backend::{FileBackend, StorageBackend};
use super::cache::DocumentCache;
use super::persistence::{PersistedDocument, load_document_from_bytes, save_document_to_bytes};
use crate::Error;
use crate::error::Result;

const META_KEY: &str = "_meta";
const DEFAULT_CACHE_SIZE: usize = 100;

/// Lightweight metadata entry for the async workspace index.
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

/// Options for async workspace creation.
#[derive(Debug, Clone)]
pub struct WorkspaceOptions {
    /// LRU cache size (default: 100).
    pub cache_size: usize,
}

impl Default for WorkspaceOptions {
    fn default() -> Self {
        Self {
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
}

/// Inner state for the async workspace.
struct WorkspaceInner {
    /// Storage backend.
    backend: Arc<dyn StorageBackend>,
    /// Root path (for file-based backends).
    root: Option<PathBuf>,
    /// Document metadata index.
    meta_index: HashMap<String, DocumentMetaEntry>,
    /// LRU cache for loaded documents.
    cache: DocumentCache,
    /// Cross-document relationship graph (cached).
    document_graph: Option<crate::graph::DocumentGraph>,
}

/// An async workspace for managing indexed documents.
///
/// Uses `tokio::sync::RwLock` for async-safe concurrent access.
/// All operations are async and can be safely called from multiple tasks.
///
/// # Thread Safety
///
/// The async workspace is fully thread-safe and can be cloned cheaply
/// (it uses `Arc` internally).
#[derive(Clone)]
pub struct Workspace {
    inner: Arc<RwLock<WorkspaceInner>>,
}

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace").finish()
    }
}

impl Workspace {
    /// Create a new async workspace with a storage backend.
    pub async fn with_backend(backend: Arc<dyn StorageBackend>) -> Result<Self> {
        Self::with_backend_and_options(backend, WorkspaceOptions::default()).await
    }

    /// Create an async workspace with backend and options.
    pub async fn with_backend_and_options(
        backend: Arc<dyn StorageBackend>,
        options: WorkspaceOptions,
    ) -> Result<Self> {
        let mut inner = WorkspaceInner {
            backend,
            root: None,
            meta_index: HashMap::new(),
            cache: DocumentCache::with_capacity(options.cache_size),
            document_graph: None,
        };

        Self::load_meta_index(&mut inner)?;

        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
        })
    }

    /// Create a new file-based async workspace at the given path.
    pub async fn new(path: impl Into<PathBuf>) -> Result<Self> {
        Self::with_options(path, WorkspaceOptions::default()).await
    }

    /// Create a new async workspace with custom cache size.
    pub async fn with_cache_size(path: impl Into<PathBuf>, cache_size: usize) -> Result<Self> {
        Self::with_options(
            path,
            WorkspaceOptions {
                cache_size,
                ..Default::default()
            },
        )
        .await
    }

    /// Create a new async workspace with custom options.
    pub async fn with_options(path: impl Into<PathBuf>, options: WorkspaceOptions) -> Result<Self> {
        let root = path.into();
        let backend = Arc::new(FileBackend::new(&root)?);

        let mut inner = WorkspaceInner {
            backend,
            root: Some(root),
            meta_index: HashMap::new(),
            cache: DocumentCache::with_capacity(options.cache_size),
            document_graph: None,
        };

        Self::load_meta_index(&mut inner)?;

        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
        })
    }

    /// Get the workspace root path (if file-based).
    pub async fn path(&self) -> Option<PathBuf> {
        let inner = self.inner.read().await;
        inner.root.clone()
    }

    /// List all document IDs in the workspace.
    pub async fn list_documents(&self) -> Vec<String> {
        let inner = self.inner.read().await;
        inner.meta_index.keys().cloned().collect()
    }

    /// Get metadata for a document.
    pub async fn get_meta(&self, id: &str) -> Option<DocumentMetaEntry> {
        let inner = self.inner.read().await;
        inner.meta_index.get(id).cloned()
    }

    /// Check if a document exists.
    pub async fn contains(&self, id: &str) -> bool {
        let inner = self.inner.read().await;
        inner.meta_index.contains_key(id)
    }

    /// Add a document to the workspace.
    pub async fn add(&self, doc: &PersistedDocument) -> Result<()> {
        let mut inner = self.inner.write().await;

        let doc_id = doc.meta.id.clone();
        let key = Self::doc_key(&doc_id);

        // Serialize and save via backend
        let bytes = save_document_to_bytes(doc)?;
        inner.backend.put(&key, &bytes)?;

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
            page_count: if doc.pages.is_empty() {
                None
            } else {
                Some(doc.pages.len())
            },
            line_count: doc.meta.line_count,
        };

        inner.meta_index.insert(doc_id.clone(), meta_entry);
        Self::save_meta_index(&inner)?;

        // Remove from cache if present
        let _ = inner.cache.remove(&doc_id);

        info!("Saved document {} to async workspace", doc_id);

        // Invalidate document graph since documents changed
        inner.document_graph = None;

        Ok(())
    }

    /// Load a document from the workspace.
    ///
    /// Uses LRU cache: returns cached version if available,
    /// otherwise loads from backend and caches it.
    pub async fn load(&self, id: &str) -> Result<Option<PersistedDocument>> {
        // First check if document exists (read lock)
        {
            let inner = self.inner.read().await;
            if !inner.meta_index.contains_key(id) {
                return Ok(None);
            }

            // Check LRU cache
            if let Some(cached) = inner.cache.get(id)? {
                debug!("Cache hit for document {}", id);
                return Ok(Some(cached));
            }
        }

        // Load from backend (need read lock for backend access)
        let inner = self.inner.read().await;
        let key = Self::doc_key(id);

        match inner.backend.get(&key)? {
            Some(bytes) => {
                let doc = load_document_from_bytes(&bytes)?;

                // Note: We can't modify the cache with only a read lock
                // For now, we return the document without caching
                // A more sophisticated implementation would use a separate cache structure

                debug!("Loaded document {} from backend", id);
                Ok(Some(doc))
            }
            None => {
                warn!("Document {} in meta index but not in backend", id);
                Ok(None)
            }
        }
    }

    /// Load a document and cache it (requires write lock for caching).
    pub async fn load_and_cache(&self, id: &str) -> Result<Option<PersistedDocument>> {
        // First check if document exists (read lock)
        {
            let inner = self.inner.read().await;
            if !inner.meta_index.contains_key(id) {
                return Ok(None);
            }

            // Check LRU cache
            if let Some(cached) = inner.cache.get(id)? {
                debug!("Cache hit for document {}", id);
                return Ok(Some(cached));
            }
        }

        // Load from backend and cache (write lock)
        let inner = self.inner.write().await;
        let key = Self::doc_key(id);

        match inner.backend.get(&key)? {
            Some(bytes) => {
                let doc = load_document_from_bytes(&bytes)?;

                // Add to cache
                inner.cache.put(id.to_string(), doc.clone())?;

                debug!("Loaded and cached document {}", id);
                Ok(Some(doc))
            }
            None => {
                warn!("Document {} in meta index but not in backend", id);
                Ok(None)
            }
        }
    }

    /// Remove a document from the workspace.
    pub async fn remove(&self, id: &str) -> Result<bool> {
        let mut inner = self.inner.write().await;

        if !inner.meta_index.contains_key(id) {
            return Ok(false);
        }

        let key = Self::doc_key(id);
        inner.backend.delete(&key)?;

        inner.meta_index.remove(id);

        // Remove from cache
        let _ = inner.cache.remove(id);

        Self::save_meta_index(&inner)?;

        info!("Removed document {} from async workspace", id);

        // Invalidate document graph since documents changed
        inner.document_graph = None;

        Ok(true)
    }

    /// Get the number of documents in the workspace.
    pub async fn len(&self) -> usize {
        let inner = self.inner.read().await;
        inner.meta_index.len()
    }

    /// Check if the workspace is empty.
    pub async fn is_empty(&self) -> bool {
        let inner = self.inner.read().await;
        inner.meta_index.is_empty()
    }

    /// Find a document ID by its source path.
    ///
    /// Returns the first document whose `source_path` matches.
    /// Used for incremental indexing to check if a file has already been indexed.
    pub async fn find_by_source_path(&self, path: &std::path::Path) -> Option<String> {
        let target = path.to_string_lossy().to_string();
        let inner = self.inner.read().await;
        for (_, entry) in &inner.meta_index {
            if entry.path.as_deref() == Some(target.as_str()) {
                return Some(entry.id.clone());
            }
        }
        None
    }

    /// Get the number of items currently in the LRU cache.
    pub async fn cache_len(&self) -> usize {
        let inner = self.inner.read().await;
        inner.cache.len()
    }

    /// Get cache utilization (0.0 to 1.0).
    pub async fn cache_utilization(&self) -> f64 {
        let inner = self.inner.read().await;
        inner.cache.utilization()
    }

    /// Get cache statistics.
    pub async fn cache_stats(&self) -> super::cache::CacheStats {
        let inner = self.inner.read().await;
        inner.cache.stats()
    }

    /// Clear the LRU cache.
    pub async fn clear_cache(&self) -> Result<()> {
        let inner = self.inner.write().await;
        inner.cache.clear()?;
        debug!("Cleared async document cache");
        Ok(())
    }

    // =========================================================================
    // Document Graph Methods
    // =========================================================================

    /// Storage key for the document graph.
    const GRAPH_KEY: &'static str = "_graph";

    /// Get the document graph, loading from backend if not cached.
    pub async fn get_graph(&self) -> Result<Option<crate::graph::DocumentGraph>> {
        // Check cache first
        {
            let inner = self.inner.read().await;
            if inner.document_graph.is_some() {
                return Ok(inner.document_graph.clone());
            }
        }

        // Load from backend
        let inner = self.inner.read().await;
        match inner.backend.get(Self::GRAPH_KEY)? {
            Some(bytes) => {
                let graph: crate::graph::DocumentGraph =
                    serde_json::from_slice(&bytes).map_err(|e| {
                        crate::Error::Serialization(format!("Failed to deserialize graph: {}", e))
                    })?;
                debug!("Loaded document graph from backend");
                Ok(Some(graph))
            }
            None => Ok(None),
        }
    }

    /// Persist the document graph to the backend.
    pub async fn set_graph(&self, graph: &crate::graph::DocumentGraph) -> Result<()> {
        let mut inner = self.inner.write().await;
        let bytes = serde_json::to_vec(graph).map_err(|e| {
            crate::Error::Serialization(format!("Failed to serialize graph: {}", e))
        })?;
        inner.backend.put(Self::GRAPH_KEY, &bytes)?;
        inner.document_graph = Some(graph.clone());
        info!(
            "Persisted document graph ({} nodes, {} edges)",
            graph.node_count(),
            graph.edge_count()
        );
        Ok(())
    }

    /// Invalidate the cached document graph (e.g. after add/remove).
    pub async fn invalidate_graph(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.document_graph = None;
        // Also remove from backend so stale graphs don't persist
        let _ = inner.backend.delete(Self::GRAPH_KEY);
        debug!("Invalidated document graph cache");
        Ok(())
    }

    /// Get the storage key for a document.
    fn doc_key(id: &str) -> String {
        format!("doc:{}", id)
    }

    /// Load the meta index from backend.
    fn load_meta_index(inner: &mut WorkspaceInner) -> Result<()> {
        match inner.backend.get(META_KEY)? {
            Some(bytes) => {
                let meta: HashMap<String, DocumentMetaEntry> = serde_json::from_slice(&bytes)
                    .map_err(|e| Error::Parse(format!("Failed to parse meta index: {}", e)))?;
                inner.meta_index = meta;
                info!(
                    "Loaded {} document(s) from async workspace index",
                    inner.meta_index.len()
                );
            }
            None => {
                // Try to rebuild from existing keys
                Self::rebuild_meta_index(inner)?;
            }
        }
        Ok(())
    }

    /// Save the meta index to backend.
    fn save_meta_index(inner: &WorkspaceInner) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&inner.meta_index)
            .map_err(|e| Error::Parse(format!("Failed to serialize meta index: {}", e)))?;
        inner.backend.put(META_KEY, &bytes)?;
        Ok(())
    }

    /// Rebuild the meta index from existing documents.
    fn rebuild_meta_index(inner: &mut WorkspaceInner) -> Result<()> {
        let keys = inner.backend.keys()?;
        let doc_keys: Vec<_> = keys.iter().filter(|k| k.starts_with("doc:")).collect();

        for key in doc_keys {
            if let Some(bytes) = inner.backend.get(key)? {
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
                        page_count: if doc.pages.is_empty() {
                            None
                        } else {
                            Some(doc.pages.len())
                        },
                        line_count: doc.meta.line_count,
                    };
                    inner.meta_index.insert(doc_id, meta_entry);
                }
            }
        }

        if !inner.meta_index.is_empty() {
            Self::save_meta_index(inner)?;
            info!(
                "Rebuilt async index from {} document(s)",
                inner.meta_index.len()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentTree;

    fn create_test_doc(id: &str) -> PersistedDocument {
        let meta = super::super::persistence::DocumentMeta::new(id, "Test Doc", "md");
        let tree = DocumentTree::new("Root", "Content");
        PersistedDocument::new(meta, tree)
    }
}
