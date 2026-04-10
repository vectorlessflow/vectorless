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
use crate::index::{PipelineExecutor, PipelineOptions};
use crate::index::incremental::{self, IndexAction};
use crate::retrieval::{PipelineRetriever, RetrieveEventReceiver};
use crate::storage::Workspace;
use crate::utils::fingerprint::Fingerprint;
use crate::{DocumentTree, Error};

use super::events::EventEmitter;
use super::index_context::{IndexContext, IndexSource};
use super::indexer::IndexerClient;
use super::query_context::{QueryContext, QueryScope};
use super::retriever::RetrieverClient;
use super::types::{DocumentInfo, FailedItem, IndexItem, IndexMode, IndexResult, QueryResult, QueryResultItem};
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
        if ctx.is_empty() {
            return Err(Error::Config("No document sources provided".to_string()));
        }

        let mut items = Vec::with_capacity(ctx.len());
        let mut failed = Vec::new();

        for source in &ctx.sources {
            let source_label = source.to_string();

            match self.resolve_index_action(source, &ctx.options).await? {
                IndexAction::Skip(skip_info) => {
                    info!("Skipped (unchanged): {}", source_label);
                    items.push(IndexItem::new(
                        skip_info.doc_id,
                        skip_info.name,
                        skip_info.format,
                        skip_info.description,
                        skip_info.page_count,
                    ));
                }
                IndexAction::FullIndex => {
                    match self
                        .indexer
                        .index(source, ctx.name.as_deref(), &ctx.options)
                        .await
                    {
                        Ok(doc) => {
                            let pipeline_options = self.build_pipeline_options(&ctx.options, doc.format);
                            let item = IndexItem::new(
                                doc.id.clone(),
                                doc.name.clone(),
                                doc.format.clone(),
                                doc.description.clone(),
                                doc.page_count,
                            );

                            let persisted = self.indexer.to_persisted_with_options(doc, &pipeline_options);

                            if let Some(ref workspace) = self.workspace {
                                if let Err(e) = workspace.save(&persisted).await {
                                    failed.push(FailedItem::new(&source_label, e.to_string()));
                                    continue;
                                }
                            }

                            info!("Indexed document: {}", item.doc_id);
                            items.push(item);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to index {}: {}", source_label, e);
                            failed.push(FailedItem::new(&source_label, e.to_string()));
                        }
                    }
                }
                IndexAction::IncrementalUpdate { old_tree, existing_id } => {
                    let existing_tree = old_tree;
                    info!("Incremental update for: {}", source_label);
                    match self
                        .indexer
                        .index_with_existing(source, ctx.name.as_deref(), &ctx.options, Some(&existing_tree))
                        .await
                    {
                        Ok(mut doc) => {
                            // Reuse the same doc_id for incremental updates
                            doc.id = existing_id.clone();
                            let pipeline_options = self.build_pipeline_options(&ctx.options, doc.format);
                            let item = IndexItem::new(
                                doc.id.clone(),
                                doc.name.clone(),
                                doc.format.clone(),
                                doc.description.clone(),
                                doc.page_count,
                            );

                            let persisted = self.indexer.to_persisted_with_options(doc, &pipeline_options);

                            if let Some(ref workspace) = self.workspace {
                                // Remove old version first
                                let _ = workspace.remove(&existing_id).await;
                                if let Err(e) = workspace.save(&persisted).await {
                                    failed.push(FailedItem::new(&source_label, e.to_string()));
                                    continue;
                                }
                            }

                            info!("Incrementally updated: {}", item.doc_id);
                            items.push(item);
                        }
                        Err(e) => {
                            tracing::warn!("Incremental update failed for {}: {}", source_label, e);
                            failed.push(FailedItem::new(&source_label, e.to_string()));
                        }
                    }
                }
            }
        }

        // If everything failed, return error
        if items.is_empty() && !failed.is_empty() {
            return Err(Error::Config(format!(
                "All {} source(s) failed to index",
                failed.len()
            )));
        }

        Ok(IndexResult::with_partial(items, failed))
    }

    // ============================================================
    // Document Querying
    // ============================================================

    /// Query documents.
    ///
    /// Accepts a [`QueryContext`] that specifies the query text and scope
    /// (single document, multiple documents, or entire workspace).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vectorless::client::{EngineBuilder, QueryContext};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let engine = EngineBuilder::new()
    ///     .with_workspace("./data")
    ///     .build()
    ///     .await?;
    ///
    /// // Single document
    /// let result = engine.query(
    ///     QueryContext::new("What is the total revenue?")
    ///         .with_doc_id("doc-123")
    /// ).await?;
    ///
    /// if let Some(item) = result.single() {
    ///     println!("Answer: {}", item.content);
    /// }
    ///
    /// // Entire workspace
    /// let result = engine.query(
    ///     QueryContext::new("Summarize all documents")
    /// ).await?;
    /// for item in &result.items {
    ///     println!("{}: score={}", item.doc_id, item.score);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query(&self, ctx: QueryContext) -> Result<QueryResult> {
        let doc_ids = self.resolve_scope(&ctx.scope).await?;
        let options = ctx.to_retrieve_options(&self.config);

        let mut items = Vec::with_capacity(doc_ids.len());
        let mut failed = Vec::new();

        for doc_id in doc_ids {
            let tree = match self.get_structure(&doc_id).await {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("Skipping document {}: {}", doc_id, e);
                    failed.push(FailedItem::new(&doc_id, e.to_string()));
                    continue;
                }
            };

            match self.retriever.query(&tree, &ctx.query, &options).await {
                Ok(mut result) => {
                    result.doc_id = doc_id;
                    items.push(result);
                }
                Err(e) => {
                    tracing::warn!("Query failed for {}: {}", doc_id, e);
                    failed.push(FailedItem::new(&doc_id, e.to_string()));
                }
            }
        }

        // If everything failed, return error
        if items.is_empty() && !failed.is_empty() {
            return Err(Error::Config(format!(
                "Query failed for all {} document(s)",
                failed.len()
            )));
        }

        Ok(QueryResult::with_partial(items, failed))
    }

    /// Query a document with streaming results.
    ///
    /// Returns a [`RetrieveEventReceiver`] that yields [`RetrieveEvent`](crate::retrieval::RetrieveEvent)s
    /// as the retrieval pipeline progresses through each stage.
    ///
    /// Only supports single-document scope (via `with_doc_id`).
    pub async fn query_stream(&self, ctx: QueryContext) -> Result<RetrieveEventReceiver> {
        let doc_id = match &ctx.scope {
            QueryScope::Single(id) => id.clone(),
            _ => return Err(Error::Config("query_stream requires a single doc_id".to_string())),
        };

        let tree = self.get_structure(&doc_id).await?;
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

    /// Resolve QueryScope into a list of document IDs.
    async fn resolve_scope(&self, scope: &QueryScope) -> Result<Vec<String>> {
        match scope {
            QueryScope::Single(id) => Ok(vec![id.clone()]),
            QueryScope::Multiple(ids) => Ok(ids.clone()),
            QueryScope::Workspace => {
                let docs = self.list().await?;
                if docs.is_empty() {
                    return Err(Error::Config("Workspace is empty".to_string()));
                }
                Ok(docs.into_iter().map(|d| d.id).collect())
            }
        }
    }

    /// Build pipeline options from client IndexOptions and detected format.
    fn build_pipeline_options(
        &self,
        options: &super::types::IndexOptions,
        format: crate::parser::DocumentFormat,
    ) -> PipelineOptions {
        use crate::index::SummaryStrategy;
        PipelineOptions {
            mode: match format {
                crate::parser::DocumentFormat::Markdown => crate::index::IndexMode::Markdown,
                crate::parser::DocumentFormat::Pdf => crate::index::IndexMode::Pdf,
                crate::parser::DocumentFormat::Html => crate::index::IndexMode::Html,
                crate::parser::DocumentFormat::Docx => crate::index::IndexMode::Docx,
            },
            generate_ids: options.generate_ids,
            summary_strategy: if options.generate_summaries {
                SummaryStrategy::full()
            } else {
                SummaryStrategy::none()
            },
            generate_description: options.generate_description,
            ..Default::default()
        }
    }

    /// Resolve what action to take for a source.
    async fn resolve_index_action(
        &self,
        source: &IndexSource,
        options: &super::types::IndexOptions,
    ) -> Result<IndexAction> {
        let workspace = match self.workspace {
            Some(ref ws) => ws,
            None => return Ok(IndexAction::FullIndex),
        };

        // Force mode always re-indexes from scratch
        if options.mode == IndexMode::Force {
            return Ok(IndexAction::FullIndex);
        }

        // Only path sources support incremental indexing
        let path = match source {
            IndexSource::Path(p) => p,
            _ => return Ok(IndexAction::FullIndex),
        };

        // Find if this file has already been indexed
        let existing_id = match workspace.find_by_source_path(path).await {
            Some(id) => id,
            None => return Ok(IndexAction::FullIndex), // New file
        };

        // Default mode: skip if already indexed (no content check)
        if options.mode == IndexMode::Default {
            let info = workspace.get_document_info(&existing_id).await?;
            let (name, format_str, desc, pages) = match info {
                Some(i) => (i.name, i.format, i.description, i.page_count),
                None => (String::new(), String::new(), None, None),
            };
            return Ok(IndexAction::Skip(incremental::SkipInfo {
                doc_id: existing_id,
                name,
                format: crate::parser::DocumentFormat::from_extension(&format_str)
                    .unwrap_or(crate::parser::DocumentFormat::Markdown),
                description: desc,
                page_count: pages,
            }));
        }

        // Incremental mode: load stored document and delegate to resolver
        let current_bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return Ok(IndexAction::FullIndex),
        };

        let stored_doc = match workspace.load(&existing_id).await? {
            Some(d) => d,
            None => return Ok(IndexAction::FullIndex),
        };

        let format = crate::parser::DocumentFormat::from_extension(&stored_doc.meta.format)
            .unwrap_or(crate::parser::DocumentFormat::Markdown);
        let pipeline_options = self.build_pipeline_options(options, format);

        // If logic fingerprint changed, remove old doc before full reprocess
        let action = incremental::resolve_action(
            &current_bytes,
            &stored_doc,
            &pipeline_options,
            format,
        );

        if matches!(action, IndexAction::FullIndex) {
            let _ = workspace.remove(&existing_id).await;
        }

        Ok(action)
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
