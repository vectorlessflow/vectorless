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
//! use vectorless::client::{EngineBuilder, IndexContext};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a client
//! let client = EngineBuilder::new()
//!     .with_workspace("./my_workspace")
//!     .build()
//!     .await?;
//!
//! // Index a document from file
//! let doc_id = client.index(IndexContext::from_path("./document.md")).await?;
//!
//! // Index HTML content
//! let html = "<html><body><h1>Title</h1><p>Content</p></body></html>";
//! let doc_id2 = client.index(
//!     IndexContext::from_content(html, vectorless::parser::DocumentFormat::Html)
//!         .with_name("webpage")
//! ).await?;
//!
//! // Query the document
//! let result = client.query(&doc_id, "What is this?").await?;
//!
//! println!("Found: {}", result.content);
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use tracing::info;

use crate::config::Config;
use crate::error::Result;
use crate::index::PipelineExecutor;
use crate::retrieval::{PipelineRetriever, RetrieveOptions};
use crate::storage::Workspace;
use crate::{DocumentTree, Error};

use super::context::ClientContext;
use super::events::EventEmitter;
use super::index_context::IndexContext;
use super::indexer::IndexerClient;
use super::retriever::RetrieverClient;
use super::session::Session;
use super::types::{DocumentInfo, QueryResult};
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
    // ============================================================
    // Constructor (for Builder)
    // ============================================================

    /// Create a new client with the given components.
    pub(crate) async fn with_components(
        config: Config,
        workspace: Workspace,
        retriever: PipelineRetriever,
        executor: PipelineExecutor,
    ) -> Result<Self> {
        let config = Arc::new(config);
        let events = EventEmitter::new();

        // Create indexer client
        let indexer = IndexerClient::new(executor).with_events(events.clone());

        // Create retriever client
        let retriever =
            RetrieverClient::new(retriever, Arc::clone(&config)).with_events(events.clone());

        // Create workspace client
        let workspace_client = WorkspaceClient::new(workspace)
            .await
            .with_events(events.clone());

        Ok(Self {
            config,
            indexer,
            retriever,
            workspace: Some(workspace_client),
            events,
        })
    }

    // ============================================================
    // Document Indexing
    // ============================================================

    /// Index a document.
    ///
    /// This is the main entry point for indexing documents. The [`IndexContext`]
    /// parameter specifies the source (file path, content string, or bytes)
    /// and indexing options.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The index context containing source and options
    ///
    /// # Returns
    ///
    /// A unique document ID string.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file does not exist (for path sources)
    /// - The file format is not supported
    /// - The pipeline execution fails
    ///
    /// # Example
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
    /// // From file
    /// let id1 = engine.index(IndexContext::from_path("./doc.md")).await?;
    ///
    /// // From content
    /// let html = "<html><body><h1>Title</h1></body></html>";
    /// let id2 = engine.index(
    ///     IndexContext::from_content(html, DocumentFormat::Html)
    ///         .with_name("webpage")
    /// ).await?;
    ///
    /// // From bytes with force mode
    /// let pdf_bytes = std::fs::read("./doc.pdf")?;
    /// let id3 = engine.index(
    ///     IndexContext::from_bytes(pdf_bytes, DocumentFormat::Pdf)
    ///         .with_mode(IndexMode::Force)
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn index(&self, ctx: IndexContext) -> Result<String> {
        println!("Indexing...");
        println!("ctx: {:?}", ctx);
        
        let doc = self.indexer.index(ctx).await?;
        let persisted = self.indexer.to_persisted(doc);

        // Save to workspace if configured
        if let Some(ref workspace) = self.workspace {
            workspace.save(&persisted).await?;
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
        let tree = self.get_structure(doc_id).await?;

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
        let tree = self.get_structure(doc_id).await?;

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

        let mut result = self
            .retriever
            .query_with_context(&tree, question, &options, ctx)
            .await?;
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
    pub async fn session(&self) -> Session {
        let workspace = match &self.workspace {
            Some(ws) => ws.clone(),
            None => {
                // Create a temporary workspace if none configured
                let async_ws = Workspace::new("./temp_workspace")
                    .await
                    .expect("Failed to create temp workspace");
                WorkspaceClient::new(async_ws).await
            }
        };

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
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace operation fails.
    pub async fn list_documents(&self) -> Result<Vec<DocumentInfo>> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.list().await
    }

    /// Get document structure (tree).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No workspace is configured
    /// - The document is not found
    pub async fn get_structure(&self, doc_id: &str) -> Result<DocumentTree> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        let doc = workspace
            .load(doc_id)
            .await?
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
    pub async fn get_page_content(&self, doc_id: &str, pages: &str) -> Result<String> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        let doc = workspace
            .load(doc_id)
            .await?
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
    pub async fn load(&self, doc_id: &str) -> Result<bool> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        if !workspace.exists(doc_id).await? {
            return Ok(false);
        }

        let _ = workspace.load(doc_id).await?;
        Ok(true)
    }

    /// Remove a document from the workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub async fn remove(&self, doc_id: &str) -> Result<bool> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.remove(doc_id).await
    }

    /// Check if a document exists in the workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub async fn exists(&self, doc_id: &str) -> Result<bool> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.exists(doc_id).await
    }

    /// Get metadata for a document.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub async fn get_metadata(&self, doc_id: &str) -> Result<Option<DocumentInfo>> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.get_document_info(doc_id).await
    }

    /// Remove multiple documents from the workspace.
    ///
    /// Returns the number of documents successfully removed.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub async fn batch_remove(&self, doc_ids: &[&str]) -> Result<usize> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.batch_remove(doc_ids).await
    }

    /// Remove all documents from the workspace.
    ///
    /// Returns the number of documents removed.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub async fn clear(&self) -> Result<usize> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.clear().await
    }

    /// Get the number of indexed documents.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace operation fails.
    pub async fn len(&self) -> Result<usize> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        Ok(workspace.len().await)
    }

    /// Check if there are no documents.
    ///
    /// # Errors
    ///
    /// Returns an error if the workspace operation fails.
    pub async fn is_empty(&self) -> Result<bool> {
        Ok(self.len().await? == 0)
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

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("has_workspace", &self.workspace.is_some())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::super::EngineBuilder;

    #[test]
    fn test_engine_builder() {
        let builder = EngineBuilder::new();
        // Builder exists
        let _ = builder;
    }
}
