// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Main Engine client - the entry point for vectorless.
//!
//! This module provides the main client for document indexing and retrieval.
//! The Engine is an orchestrator that delegates to specialized sub-clients.
//!
//! # Architecture
//!
//! ```text
//! Engine (Orchestrator)
//! ├── IndexerClient   → Document indexing
//! ├── RetrieverClient → Query and retrieval
//! ├── WorkspaceClient → Document persistence
//! └── EventEmitter    → Progress and events
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::client::{Engine, EngineBuilder};
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::domain::Result<()> {
//! // Create a client
//! let client = EngineBuilder::new()
//!     .with_workspace("./my_workspace")
//!     .build()?;
//!
//! // Index a document
//! let doc_id = client.index("./document.md").await?;
//!
//! // Query the document
//! let result = client.query(&doc_id, "What is this?").await?;
//!
//! println!("Found: {}", result.content);
//! # Ok(())
//! # }
//! ```

use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use tracing::info;

use crate::config::Config;
use crate::error::Result;
use crate::{DocumentTree, Error};
use crate::index::PipelineExecutor;
use crate::retrieval::{PipelineRetriever, RetrieveOptions};
use crate::storage::Workspace;

use super::context::ClientContext;
use super::events::EventEmitter;
use super::indexer::IndexerClient;
use super::retriever::RetrieverClient;
use super::session::Session;
use super::types::{DocumentInfo, IndexOptions, QueryResult};
use super::workspace::WorkspaceClient;

/// The main Engine client.
///
/// Provides high-level operations for document indexing and retrieval.
/// Uses interior mutability to allow sharing across async tasks.
///
/// # Cloning
///
/// Cloning is cheap - it only increments reference counts (`Arc`). All clones
/// share the same underlying resources.
///
/// # Thread Safety
///
/// The client is `Clone + Send + Sync` and can be safely shared across threads.
pub struct Engine {
    /// Configuration (immutable, shared).
    config: Arc<Config>,

    /// Indexer client for document indexing.
    indexer: IndexerClient,

    /// Retriever client for queries.
    retriever: RetrieverClient,

    /// Workspace client for persistence.
    workspace: Option<WorkspaceClient>,

    /// Event emitter.
    events: EventEmitter,
}

impl Engine {
    /// Create a builder for custom configuration.
    #[must_use]
    pub fn builder() -> super::EngineBuilder {
        super::EngineBuilder::new()
    }

    /// Create a new client with default configuration.
    ///
    /// Note: Prefer using [`Engine::builder()`] for more control.
    fn new() -> Result<Self> {
        let config = Config::default();
        Self::with_components(
            config,
            None,
            PipelineRetriever::new(),
            PipelineExecutor::new(),
        )
    }

    // ============================================================
    // Constructor (for Builder)
    // ============================================================

    /// Create a new client with the given components.
    pub(crate) fn with_components(
        config: Config,
        workspace: Option<Workspace>,
        retriever: PipelineRetriever,
        executor: PipelineExecutor,
    ) -> Result<Self> {
        let config = Arc::new(config);
        let events = EventEmitter::new();

        // Create indexer client
        let indexer = IndexerClient::new(executor)
            .with_events(events.clone());

        // Create retriever client
        let retriever = RetrieverClient::new(retriever, Arc::clone(&config))
            .with_events(events.clone());

        // Create workspace client (if workspace provided)
        let workspace_client = workspace.map(|ws| {
            WorkspaceClient::new(ws).with_events(events.clone())
        });

        Ok(Self {
            config,
            indexer,
            retriever,
            workspace: workspace_client,
            events,
        })
    }

    // ============================================================
    // Document Indexing
    // ============================================================

    /// Index a document from a file path.
    ///
    /// Returns a unique document ID.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file does not exist
    /// - The file format is not supported
    /// - The pipeline execution fails
    pub async fn index(&self, path: impl AsRef<Path>) -> Result<String> {
        self.index_with_options(path, IndexOptions::default()).await
    }

    /// Index a document with custom options.
    ///
    /// # Errors
    ///
    /// See [`Engine::index`].
    pub async fn index_with_options(
        &self,
        path: impl AsRef<Path>,
        options: IndexOptions,
    ) -> Result<String> {
        let doc = self.indexer.index_with_options(path, options).await?;
        let persisted = self.indexer.to_persisted(doc);

        // Save to workspace if configured
        if let Some(ref workspace) = self.workspace {
            workspace.save(&persisted)?;
        }

        let doc_id = persisted.meta.id.clone();
        info!("Indexed document: {}", doc_id);
        Ok(doc_id)
    }

    // ============================================================
    // Document Querying
    // ============================================================

    /// Query a document.
    ///
    /// Uses the adaptive retriever to find relevant content.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No workspace is configured
    /// - The document is not found
    /// - The retrieval fails
    pub async fn query(&self, doc_id: &str, question: &str) -> Result<QueryResult> {
        let tree = self.get_structure(doc_id)?;

        let options = RetrieveOptions::new()
            .with_top_k(self.config.retrieval.top_k)
            .with_include_content(true)
            .with_include_summaries(true);

        let mut result = self.retriever.query(&tree, question, &options).await?;
        result.doc_id = doc_id.to_string();

        Ok(result)
    }

    /// Query a document with context.
    ///
    /// Allows request-specific configuration overrides.
    pub async fn query_with_context(
        &self,
        doc_id: &str,
        question: &str,
        ctx: &ClientContext,
    ) -> Result<QueryResult> {
        let tree = self.get_structure(doc_id)?;

        let mut options = RetrieveOptions::new()
            .with_top_k(self.config.retrieval.top_k)
            .with_include_content(true)
            .with_include_summaries(true);

        // Apply context overrides
        if let Some(top_k) = ctx.config.top_k {
            options.top_k = top_k;
        }
        if let Some(token_budget) = ctx.config.token_budget {
            options.max_tokens = token_budget;
        }

        let mut result = self.retriever.query_with_context(&tree, question, &options, ctx).await?;
        result.doc_id = doc_id.to_string();

        Ok(result)
    }

    // ============================================================
    // Session Management
    // ============================================================

    /// Create a session for multi-document operations.
    ///
    /// Sessions provide:
    /// - Automatic caching of document trees
    /// - Cross-document queries
    /// - Session statistics
    pub fn session(&self) -> Session {
        let workspace = self.workspace.clone().unwrap_or_else(|| {
            WorkspaceClient::from_arc(
                Arc::new(RwLock::new(Workspace::open("./temp_workspace").unwrap())),
                self.events.clone(),
            )
        });

        Session::new(
            self.indexer.clone(),
            self.retriever.clone(),
            workspace,
            self.events.clone(),
        )
    }

    // ============================================================
    // Document Retrieval
    // ============================================================

    /// Get a list of all indexed documents.
    #[must_use]
    pub fn list_documents(&self) -> Vec<DocumentInfo> {
        match &self.workspace {
            Some(workspace) => workspace.list().unwrap_or_default(),
            None => Vec::new(),
        }
    }

    /// Get document structure (tree).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No workspace is configured
    /// - The document is not found
    pub fn get_structure(&self, doc_id: &str) -> Result<DocumentTree> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        let doc = workspace.load(doc_id)?
            .ok_or_else(|| Error::DocumentNotFound(format!("Document not found: {}", doc_id)))?;

        Ok(doc.tree)
    }

    /// Get page content for PDFs.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No workspace is configured
    /// - The document is not found
    /// - No page content is available
    pub fn get_page_content(&self, doc_id: &str, pages: &str) -> Result<String> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        let doc = workspace.load(doc_id)?
            .ok_or_else(|| Error::DocumentNotFound(format!("Document not found: {}", doc_id)))?;

        if doc.pages.is_empty() {
            return Err(Error::Parse("No page content available".to_string()));
        }

        let page_nums = self.parse_page_range(pages)?;

        let mut content = String::new();
        for page_num in page_nums {
            if let Some(page) = doc.pages.iter().find(|p| p.page == page_num) {
                content.push_str(&format!("--- Page {} ---\n", page_num));
                content.push_str(&page.content);
                content.push_str("\n\n");
            }
        }

        Ok(content)
    }

    /// Parse a page range string into page numbers.
    fn parse_page_range(&self, pages: &str) -> Result<Vec<usize>> {
        let mut result = Vec::new();

        for part in pages.split(',') {
            let part = part.trim();
            if part.contains('-') {
                let range: Vec<&str> = part.split('-').collect();
                if range.len() == 2 {
                    let start: usize = range[0]
                        .parse()
                        .map_err(|_| Error::Parse(format!("Invalid page number: {}", range[0])))?;
                    let end: usize = range[1]
                        .parse()
                        .map_err(|_| Error::Parse(format!("Invalid page number: {}", range[1])))?;
                    for p in start..=end {
                        result.push(p);
                    }
                }
            } else if !part.is_empty() {
                let page: usize = part
                    .parse()
                    .map_err(|_| Error::Parse(format!("Invalid page number: {}", part)))?;
                result.push(page);
            }
        }

        Ok(result)
    }

    // ============================================================
    // Persistence Operations
    // ============================================================

    /// Load a document from the workspace into cache.
    ///
    /// This preloads the document into the LRU cache for faster access.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub fn load(&self, doc_id: &str) -> Result<bool> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        if !workspace.exists(doc_id)? {
            return Ok(false);
        }

        let _ = workspace.load(doc_id)?;
        Ok(true)
    }

    /// Remove a document from the workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub fn remove(&self, doc_id: &str) -> Result<bool> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.remove(doc_id)
    }

    /// Check if a document exists in the workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub fn exists(&self, doc_id: &str) -> Result<bool> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.exists(doc_id)
    }

    /// Get metadata for a document.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub fn get_metadata(&self, doc_id: &str) -> Result<Option<DocumentInfo>> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.get_document_info(doc_id)
    }

    /// Remove multiple documents from the workspace.
    ///
    /// Returns the number of documents successfully removed.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub fn batch_remove(&self, doc_ids: &[&str]) -> Result<usize> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.batch_remove(doc_ids)
    }

    /// Remove all documents from the workspace.
    ///
    /// Returns the number of documents removed.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub fn clear(&self) -> Result<usize> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.clear()
    }

    /// Get the number of indexed documents.
    #[must_use]
    pub fn len(&self) -> usize {
        self.workspace.as_ref().map(|w| w.len()).unwrap_or(0)
    }

    /// Check if there are no documents.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // ============================================================
    // Sub-Client Access
    // ============================================================

    /// Get the indexer client.
    pub fn indexer(&self) -> &IndexerClient {
        &self.indexer
    }

    /// Get the retriever client.
    pub fn retriever(&self) -> &RetrieverClient {
        &self.retriever
    }

    /// Get the workspace client.
    pub fn workspace(&self) -> Option<&WorkspaceClient> {
        self.workspace.as_ref()
    }

    /// Get the configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }
}

impl Clone for Engine {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            indexer: self.indexer.clone(),
            retriever: self.retriever.clone(),
            workspace: self.workspace.clone(),
            events: self.events.clone(),
        }
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new().expect("Failed to create default Engine client")
    }
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("has_workspace", &self.workspace.is_some())
            .field("doc_count", &self.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_builder() {
        let builder = Engine::builder();
        // Builder exists
        let _ = builder;
    }
}
