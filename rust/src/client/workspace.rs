// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Workspace management client.
//!
//! This module provides async CRUD operations for document persistence
//! through the workspace abstraction.
//!
//! # Example
//!
//! ```rust,ignore
//! let workspace = WorkspaceClient::new(workspace_storage).await;
//!
//! // Save a document
//! workspace.save(&doc).await?;
//!
//! // Load a document
//! let doc = workspace.load("doc-id").await?;
//!
//! // List all documents
//! for doc in workspace.list().await? {
//!     println!("{}: {}", doc.id, doc.name);
//! }
//! ```

use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::error::Result;
use crate::storage::{PersistedDocument, Workspace};

use super::events::{EventEmitter, WorkspaceEvent};
use super::types::DocumentInfo;

/// Workspace management client.
///
/// Provides async thread-safe CRUD operations for document persistence.
/// All operations are async and can be safely called from multiple tasks.
///
/// # Thread Safety
///
/// The client is fully thread-safe and can be cloned cheaply
/// (it uses `Arc` internally).
#[derive(Clone)]
pub(crate) struct WorkspaceClient {
    /// Workspace storage.
    workspace: Arc<Workspace>,

    /// Event emitter.
    events: EventEmitter,

    /// Configuration.
    config: WorkspaceClientConfig,
}

/// Workspace client configuration.
#[derive(Debug, Clone)]
pub(crate) struct WorkspaceClientConfig {
    /// Auto-save interval in seconds (None = disabled).
    pub auto_save_interval: Option<u64>,

    /// Enable verbose logging.
    pub verbose: bool,
}

impl Default for WorkspaceClientConfig {
    fn default() -> Self {
        Self {
            auto_save_interval: None,
            verbose: false,
        }
    }
}

impl WorkspaceClient {
    /// Create a new workspace client.
    pub async fn new(workspace: Workspace) -> Self {
        Self {
            workspace: Arc::new(workspace),
            events: EventEmitter::new(),
            config: WorkspaceClientConfig::default(),
        }
    }

    /// Create with event emitter.
    pub fn with_events(mut self, events: EventEmitter) -> Self {
        self.events = events;
        self
    }

    /// Create with configuration.
    pub fn with_config(mut self, config: WorkspaceClientConfig) -> Self {
        self.config = config;
        self
    }

    /// Create from an existing workspace Arc.
    pub(crate) fn from_arc(workspace: Arc<Workspace>, events: EventEmitter) -> Self {
        Self {
            workspace,
            events,
            config: WorkspaceClientConfig::default(),
        }
    }

    /// Save a document to the workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace write fails.
    pub async fn save(&self, doc: &PersistedDocument) -> Result<()> {
        let doc_id = doc.meta.id.clone();

        self.workspace.add(doc).await?;

        info!("Saved document: {}", doc_id);
        self.events.emit_workspace(WorkspaceEvent::Saved { doc_id });

        Ok(())
    }

    /// Load a document from the workspace.
    ///
    /// Returns `Ok(None)` if the document doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace read fails.
    pub async fn load(&self, doc_id: &str) -> Result<Option<PersistedDocument>> {
        if !self.workspace.contains(doc_id).await {
            return Ok(None);
        }

        let doc = self.workspace.load_and_cache(doc_id).await?;
        let cache_hit = doc.is_some();

        if let Some(ref doc) = doc {
            debug!("Loaded document: {} (cache={})", doc_id, cache_hit);
        }

        self.events.emit_workspace(WorkspaceEvent::Loaded {
            doc_id: doc_id.to_string(),
            cache_hit,
        });

        Ok(doc)
    }

    /// Remove a document from the workspace.
    ///
    /// Returns `Ok(true)` if the document was removed, `Ok(false)` if it didn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace write fails.
    pub async fn remove(&self, doc_id: &str) -> Result<bool> {
        let removed = self.workspace.remove(doc_id).await?;

        if removed {
            info!("Removed document: {}", doc_id);
            self.events.emit_workspace(WorkspaceEvent::Removed {
                doc_id: doc_id.to_string(),
            });
        }

        Ok(removed)
    }

    /// Check if a document exists in the workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace read fails.
    pub async fn exists(&self, doc_id: &str) -> Result<bool> {
        Ok(self.workspace.contains(doc_id).await)
    }

    /// List all documents in the workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace read fails.
    pub async fn list(&self) -> Result<Vec<DocumentInfo>> {
        let doc_ids = self.workspace.list_documents().await;
        let mut result = Vec::with_capacity(doc_ids.len());

        for id in &doc_ids {
            if let Some(meta) = self.workspace.get_meta(id).await {
                result.push(DocumentInfo {
                    id: meta.id,
                    name: meta.doc_name,
                    format: meta.doc_type,
                    description: meta.doc_description,
                    page_count: meta.page_count,
                    line_count: meta.line_count,
                });
            }
        }

        Ok(result)
    }

    /// Get document info by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace read fails.
    pub async fn get_document_info(&self, doc_id: &str) -> Result<Option<DocumentInfo>> {
        Ok(self
            .workspace
            .get_meta(doc_id)
            .await
            .map(|meta| DocumentInfo {
                id: meta.id,
                name: meta.doc_name,
                format: meta.doc_type,
                description: meta.doc_description,
                page_count: meta.page_count,
                line_count: meta.line_count,
            }))
    }

    /// Remove multiple documents from the workspace.
    ///
    /// Returns the number of documents successfully removed.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace write fails.
    pub async fn batch_remove(&self, doc_ids: &[&str]) -> Result<usize> {
        let mut removed = 0;

        for doc_id in doc_ids {
            if self.workspace.remove(doc_id).await? {
                removed += 1;
                self.events.emit_workspace(WorkspaceEvent::Removed {
                    doc_id: doc_id.to_string(),
                });
            }
        }

        if removed > 0 {
            info!("Batch removed {} documents", removed);
        }

        Ok(removed)
    }

    /// Clear all documents from the workspace.
    ///
    /// Returns the number of documents removed.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace write fails.
    pub async fn clear(&self) -> Result<usize> {
        let doc_ids = self.workspace.list_documents().await;
        let count = doc_ids.len();

        for doc_id in &doc_ids {
            let _ = self.workspace.remove(doc_id).await;
        }

        if count > 0 {
            info!("Cleared workspace: {} documents removed", count);
            self.events
                .emit_workspace(WorkspaceEvent::Cleared { count });
        }

        Ok(count)
    }

    /// Get workspace statistics.
    pub async fn stats(&self) -> Result<WorkspaceStats> {
        Ok(WorkspaceStats {
            document_count: self.workspace.len().await,
        })
    }

    /// Get the number of documents in the workspace.
    pub async fn len(&self) -> usize {
        self.workspace.len().await
    }

    /// Check if the workspace is empty.
    pub async fn is_empty(&self) -> bool {
        self.workspace.is_empty().await
    }

    /// Get the underlying workspace Arc (for advanced use).
    pub(crate) fn inner(&self) -> Arc<Workspace> {
        Arc::clone(&self.workspace)
    }

    /// Find a document ID by its source file path.
    ///
    /// Used for incremental indexing to check if a file has already been indexed.
    pub async fn find_by_source_path(&self, path: &std::path::Path) -> Option<String> {
        self.workspace.find_by_source_path(path).await
    }
}

/// Workspace statistics.
#[derive(Debug, Clone)]
pub(crate) struct WorkspaceStats {
    /// Number of documents in the workspace.
    pub document_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::backend::MemoryBackend;
    use std::sync::Arc as StdArc;

    #[tokio::test]
    async fn test_workspace_client_creation() {
        let backend = StdArc::new(MemoryBackend::new());
        let workspace = Workspace::with_backend(backend).await.unwrap();
        let client = WorkspaceClient::new(workspace).await;
        assert!(client.is_empty().await);
    }

    #[tokio::test]
    async fn test_workspace_stats() {
        let backend = StdArc::new(MemoryBackend::new());
        let workspace = Workspace::with_backend(backend).await.unwrap();
        let client = WorkspaceClient::new(workspace).await;

        let stats = client.stats().await.unwrap();
        assert_eq!(stats.document_count, 0);
    }
}
