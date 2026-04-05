// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Workspace management client.
//!
//! This module provides CRUD operations for document persistence
//! through the workspace abstraction.
//!
//! # Example
//!
//! ```rust,ignore
//! let workspace = WorkspaceClient::new(workspace_storage);
//!
//! // Save a document
//! workspace.save(&doc)?;
//!
//! // Load a document
//! let doc = workspace.load("doc-id")?;
//!
//! // List all documents
//! for doc in workspace.list()? {
//!     println!("{}: {}", doc.id, doc.name);
//! }
//! ```

use std::sync::{Arc, RwLock};

use tracing::{debug, info, warn};

use crate::{Error};
use crate::error::Result;
use crate::storage::{DocumentMetaEntry, PersistedDocument, Workspace};

use super::events::{EventEmitter, WorkspaceEvent};
use super::types::DocumentInfo;

/// Workspace management client.
///
/// Provides thread-safe CRUD operations for document persistence.
pub struct WorkspaceClient {
    /// Workspace storage.
    workspace: Arc<RwLock<Workspace>>,

    /// Event emitter.
    events: EventEmitter,

    /// Configuration.
    config: WorkspaceClientConfig,
}

/// Workspace client configuration.
#[derive(Debug, Clone)]
pub struct WorkspaceClientConfig {
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
    pub fn new(workspace: Workspace) -> Self {
        Self {
            workspace: Arc::new(RwLock::new(workspace)),
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
    pub(crate) fn from_arc(workspace: Arc<RwLock<Workspace>>, events: EventEmitter) -> Self {
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
    pub fn save(&self, doc: &PersistedDocument) -> Result<()> {
        let doc_id = doc.meta.id.clone();

        {
            let mut ws = self.workspace.write()
                .map_err(|_| Error::Other("Workspace lock poisoned".to_string()))?;
            ws.add(doc)?;
        }

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
    pub fn load(&self, doc_id: &str) -> Result<Option<PersistedDocument>> {
        let ws = self.workspace.read()
            .map_err(|_| Error::Other("Workspace lock poisoned".to_string()))?;

        if !ws.contains(doc_id) {
            return Ok(None);
        }

        let doc = ws.load(doc_id)?;
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
    pub fn remove(&self, doc_id: &str) -> Result<bool> {
        let removed = {
            let mut ws = self.workspace.write()
                .map_err(|_| Error::Other("Workspace lock poisoned".to_string()))?;
            ws.remove(doc_id)?
        };

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
    pub fn exists(&self, doc_id: &str) -> Result<bool> {
        let ws = self.workspace.read()
            .map_err(|_| Error::Other("Workspace lock poisoned".to_string()))?;
        Ok(ws.contains(doc_id))
    }

    /// List all documents in the workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace read fails.
    pub fn list(&self) -> Result<Vec<DocumentInfo>> {
        let ws = self.workspace.read()
            .map_err(|_| Error::Other("Workspace lock poisoned".to_string()))?;

        Ok(ws.list_documents()
            .iter()
            .filter_map(|id| ws.get_meta(id))
            .map(|meta| DocumentInfo {
                id: meta.id.clone(),
                name: meta.doc_name.clone(),
                format: meta.doc_type.clone(),
                description: meta.doc_description.clone(),
                page_count: meta.page_count,
                line_count: meta.line_count,
            })
            .collect())
    }

    /// Get document metadata without loading the full document.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace read fails.
    pub fn get_meta(&self, doc_id: &str) -> Result<Option<DocumentMetaEntry>> {
        let ws = self.workspace.read()
            .map_err(|_| Error::Other("Workspace lock poisoned".to_string()))?;
        Ok(ws.get_meta(doc_id).cloned())
    }

    /// Get document info by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace read fails.
    pub fn get_document_info(&self, doc_id: &str) -> Result<Option<DocumentInfo>> {
        let ws = self.workspace.read()
            .map_err(|_| Error::Other("Workspace lock poisoned".to_string()))?;

        Ok(ws.get_meta(doc_id).map(|meta| DocumentInfo {
            id: meta.id.clone(),
            name: meta.doc_name.clone(),
            format: meta.doc_type.clone(),
            description: meta.doc_description.clone(),
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
    pub fn batch_remove(&self, doc_ids: &[&str]) -> Result<usize> {
        let mut removed = 0;

        {
            let mut ws = self.workspace.write()
                .map_err(|_| Error::Other("Workspace lock poisoned".to_string()))?;

            for doc_id in doc_ids {
                if ws.remove(doc_id)? {
                    removed += 1;
                    self.events.emit_workspace(WorkspaceEvent::Removed {
                        doc_id: doc_id.to_string(),
                    });
                }
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
    pub fn clear(&self) -> Result<usize> {
        let doc_ids: Vec<String>;

        {
            let ws = self.workspace.read()
                .map_err(|_| Error::Other("Workspace lock poisoned".to_string()))?;
            doc_ids = ws.list_documents().iter().map(|s| s.to_string()).collect();
        }

        let count = doc_ids.len();

        {
            let mut ws = self.workspace.write()
                .map_err(|_| Error::Other("Workspace lock poisoned".to_string()))?;

            for doc_id in &doc_ids {
                let _ = ws.remove(doc_id);
            }
        }

        if count > 0 {
            info!("Cleared workspace: {} documents removed", count);
            self.events.emit_workspace(WorkspaceEvent::Cleared { count });
        }

        Ok(count)
    }

    /// Get workspace statistics.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace read fails.
    pub fn stats(&self) -> Result<WorkspaceStats> {
        let ws = self.workspace.read()
            .map_err(|_| Error::Other("Workspace lock poisoned".to_string()))?;

        Ok(WorkspaceStats {
            document_count: ws.len(),
        })
    }

    /// Get the number of documents in the workspace.
    pub fn len(&self) -> usize {
        self.workspace.read()
            .map(|ws| ws.len())
            .unwrap_or(0)
    }

    /// Check if the workspace is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the underlying workspace Arc (for advanced use).
    pub(crate) fn inner(&self) -> Arc<RwLock<Workspace>> {
        Arc::clone(&self.workspace)
    }
}

impl Clone for WorkspaceClient {
    fn clone(&self) -> Self {
        Self {
            workspace: Arc::clone(&self.workspace),
            events: self.events.clone(),
            config: self.config.clone(),
        }
    }
}

/// Workspace statistics.
#[derive(Debug, Clone)]
pub struct WorkspaceStats {
    /// Number of documents in the workspace.
    pub document_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_client_creation() {
        let workspace = Workspace::open("./test_workspace").unwrap();
        let client = WorkspaceClient::new(workspace);
        assert!(client.is_empty());
    }

    #[test]
    fn test_workspace_stats() {
        let workspace = Workspace::open("./test_workspace").unwrap();
        let client = WorkspaceClient::new(workspace);

        let stats = client.stats().unwrap();
        assert_eq!(stats.document_count, 0);
    }
}
