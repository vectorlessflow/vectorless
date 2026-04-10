// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Main Engine client - the entry point for vectorless.
//!
//! The Engine provides a unified API for document indexing and retrieval:
//!
//! - [`index`](Engine::index) — Index documents from files, content, or bytes
//! - [`query`](Engine::query) — Query documents using natural language
//! - [`query_stream`](Engine::query_stream) — Query with streaming results
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::client::{EngineBuilder, IndexContext, QueryContext};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let engine = EngineBuilder::new()
//!     .with_workspace("./data")
//!     .build()
//!     .await?;
//!
//! // Index a document
//! let result = engine.index(IndexContext::from_path("./document.md")).await?;
//! let doc_id = result.doc_id().unwrap();
//!
//! // Query
//! let result = engine.query(
//!     QueryContext::new("What is this?").with_doc_id(doc_id)
//! ).await?;
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
use crate::retrieval::{PipelineRetriever, RetrieveEventReceiver};
use crate::storage::Workspace;
use crate::{DocumentTree, Error};

use super::events::EventEmitter;
use super::index_context::IndexContext;
use super::indexer::IndexerClient;
use super::query_context::QueryContext;
use super::retriever::RetrieverClient;
use super::types::{DocumentInfo, IndexItem, IndexResult, QueryResult};
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
    /// Accepts an [`IndexContext`] that specifies the source (file path,
    /// content string, or bytes) and indexing options.
    ///
    /// Returns an [`IndexResult`] containing the indexed document metadata.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vectorless::client::{EngineBuilder, IndexContext};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let engine = EngineBuilder::new()
    ///     .with_workspace("./data")
    ///     .build()
    ///     .await?;
    ///
    /// let result = engine.index(IndexContext::from_path("./doc.md")).await?;
    /// println!("Indexed: {}", result.doc_id().unwrap());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn index(&self, ctx: IndexContext) -> Result<IndexResult> {
        let doc = self.indexer.index(ctx).await?;

        let item = IndexItem::new(doc.id.clone(), doc.name.clone(), doc.format.clone());

        let persisted = self.indexer.to_persisted(doc);

        // Save to workspace if configured
        if let Some(ref workspace) = self.workspace {
            workspace.save(&persisted).await?;
        }

        info!("Indexed document: {}", item.doc_id);
        Ok(IndexResult::new(vec![item]))
    }

    // ============================================================
    // Document Querying
    // ============================================================

    /// Query a document.
    ///
    /// Accepts a [`QueryContext`] that specifies the query text, target document,
    /// and optional retrieval parameters.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vectorless::client::{EngineBuilder, IndexContext, QueryContext};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let engine = EngineBuilder::new()
    ///     .with_workspace("./data")
    ///     .build()
    ///     .await?;
    ///
    /// let result = engine.query(
    ///     QueryContext::new("What is the total revenue?")
    ///         .with_doc_id("doc-123")
    /// ).await?;
    ///
    /// println!("Answer: {}", result.content);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query(&self, ctx: QueryContext) -> Result<QueryResult> {
        let doc_id = ctx.doc_id.as_deref().ok_or_else(|| {
            Error::Config("doc_id is required for query".to_string())
        })?;

        let tree = self.get_structure(doc_id).await?;
        let options = ctx.to_retrieve_options(&self.config);

        let mut result = self.retriever.query(&tree, &ctx.query, &options).await?;
        result.doc_id = doc_id.to_string();

        Ok(result)
    }

    /// Query a document with streaming results.
    ///
    /// Returns a [`RetrieveEventReceiver`] that yields [`RetrieveEvent`](crate::retrieval::RetrieveEvent)s
    /// as the retrieval pipeline progresses through each stage.
    pub async fn query_stream(&self, ctx: QueryContext) -> Result<RetrieveEventReceiver> {
        let doc_id = ctx.doc_id.as_deref().ok_or_else(|| {
            Error::Config("doc_id is required for query".to_string())
        })?;

        let tree = self.get_structure(doc_id).await?;
        let options = ctx.to_retrieve_options(&self.config);

        let rx = self.retriever.query_stream(&tree, &ctx.query, &options).await?;

        Ok(rx)
    }

    // ============================================================
    // Document Management
    // ============================================================

    /// Get a list of all indexed documents.
    pub async fn list(&self) -> Result<Vec<DocumentInfo>> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.list().await
    }

    /// Remove a document from the workspace.
    pub async fn remove(&self, doc_id: &str) -> Result<bool> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.remove(doc_id).await
    }

    /// Check if a document exists in the workspace.
    pub async fn exists(&self, doc_id: &str) -> Result<bool> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.exists(doc_id).await
    }

    /// Remove all documents from the workspace.
    ///
    /// Returns the number of documents removed.
    pub async fn clear(&self) -> Result<usize> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.clear().await
    }

    // ============================================================
    // Internal
    // ============================================================

    /// Get document structure (tree). Internal use only.
    pub(crate) async fn get_structure(&self, doc_id: &str) -> Result<DocumentTree> {
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
        let _ = builder;
    }
}
