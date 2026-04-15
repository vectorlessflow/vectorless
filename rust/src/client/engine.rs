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
//!     .with_key("sk-...")
//!     .with_model("gpt-4o")
//!     .build()
//!     .await?;
//!
//! // Index a document
//! let result = engine.index(IndexContext::from_path("./document.md")).await?;
//! let doc_id = result.doc_id().unwrap();
//!
//! // Query
//! let result = engine.query(
//!     QueryContext::new("What is this?").with_doc_ids(vec![doc_id.to_string()])
//! ).await?;
//!
//! println!("Found: {}", result.content);
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use tracing::info;

use crate::config::Config;
use crate::error::Result;
use crate::index::PipelineOptions;
use crate::index::incremental::{self, IndexAction};
use crate::metrics::MetricsHub;
use crate::retrieval::{PipelineRetriever, RetrieveEventReceiver};
use crate::storage::{PersistedDocument, Workspace};
use crate::{DocumentTree, Error};

use crate::events::EventEmitter;
use super::index_context::{IndexContext, IndexSource};
use super::indexer::IndexerClient;
use super::query_context::{QueryContext, QueryScope};
use super::retriever::RetrieverClient;
use super::types::{DocumentInfo, FailedItem, IndexItem, IndexMode, IndexResult, QueryResult};
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

    /// Central metrics hub for unified collection.
    metrics_hub: Arc<MetricsHub>,
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
        indexer: IndexerClient,
        events: EventEmitter,
    ) -> Result<Self> {
        let config = Arc::new(config);

        // Attach event emitter to indexer
        let indexer = indexer.with_events(events.clone());

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
            metrics_hub: Arc::new(MetricsHub::with_defaults()),
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
    ///     .with_key("sk-...")
    ///     .with_model("gpt-4o")
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

        // Single source: no need for concurrency overhead
        if ctx.sources.len() == 1 {
            let source = &ctx.sources[0];
            let (items, failed) = self
                .process_source(source, &ctx.options, ctx.name.as_deref())
                .await;
            if items.is_empty() && !failed.is_empty() {
                return Err(Error::Config(format!(
                    "All {} source(s) failed to index: {}",
                    failed.len(),
                    failed
                        .iter()
                        .map(|f| format!("{} ({})", f.source, f.error))
                        .collect::<Vec<_>>()
                        .join("; ")
                )));
            }
            if !items.is_empty() {
                if let Err(e) = self.rebuild_graph().await {
                    tracing::warn!("Graph rebuild failed: {}", e);
                }
            }
            return Ok(IndexResult::with_partial(items, failed));
        }

        // Multiple sources: parallel indexing
        let concurrency = self
            .config
            .concurrency
            .max_concurrent_requests
            .min(ctx.sources.len());

        let results: Vec<(Vec<IndexItem>, Vec<FailedItem>)> =
            futures::stream::iter(ctx.sources.iter().cloned())
                .map(|source| {
                    let options = ctx.options.clone();
                    let name = ctx.name.clone();
                    let engine = self.clone();
                    async move {
                        engine
                            .process_source(&source, &options, name.as_deref())
                            .await
                    }
                })
                .buffer_unordered(concurrency)
                .collect()
                .await;

        let mut items = Vec::new();
        let mut failed = Vec::new();
        for (ok, err) in results {
            items.extend(ok);
            failed.extend(err);
        }

        if items.is_empty() && !failed.is_empty() {
            return Err(Error::Config(format!(
                "All {} source(s) failed to index: {}",
                failed.len(),
                failed
                    .iter()
                    .map(|f| format!("{} ({})", f.source, f.error))
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }

        // Rebuild document graph after successful batch index
        if !items.is_empty() {
            if let Err(e) = self.rebuild_graph().await {
                tracing::warn!("Graph rebuild failed: {}", e);
            }
        }

        Ok(IndexResult::with_partial(items, failed))
    }

    /// Process a single source — resolve action and index.
    ///
    /// Returns `(items, failed)`.
    async fn process_source(
        &self,
        source: &IndexSource,
        options: &super::types::IndexOptions,
        name: Option<&str>,
    ) -> (Vec<IndexItem>, Vec<FailedItem>) {
        let source_label = source.to_string();

        match self.resolve_index_action(source, options).await {
            Ok(IndexAction::Skip(skip_info)) => {
                info!("Skipped (unchanged): {}", source_label);
                (
                    vec![IndexItem::new(
                        skip_info.doc_id,
                        skip_info.name,
                        skip_info.format,
                        skip_info.description,
                        skip_info.page_count,
                    )],
                    Vec::new(),
                )
            }
            Ok(IndexAction::FullIndex { existing_id }) => {
                match self.indexer.index(source, name, options).await {
                    Ok(doc) => {
                        let pipeline_options = self.build_pipeline_options(options, doc.format);
                        let metrics = doc.metrics.clone();
                        let item = IndexItem::new(
                            doc.id.clone(),
                            doc.name.clone(),
                            doc.format.clone(),
                            doc.description.clone(),
                            doc.page_count,
                        )
                        .with_source_path(
                            doc.source_path
                                .as_ref()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_default(),
                        )
                        .with_metrics_opt(metrics);
                        let persisted = self
                            .indexer
                            .to_persisted_with_options(doc, &pipeline_options);

                        if let Some(ref workspace) = self.workspace {
                            if let Err(e) = workspace.save(&persisted).await {
                                return (
                                    Vec::new(),
                                    vec![FailedItem::new(&source_label, e.to_string())],
                                );
                            }
                            // Clean up old document after successful save (atomic: save-first, then remove old)
                            if let Some(old_id) = &existing_id {
                                let _ = workspace.remove(old_id).await;
                            }
                        }

                        info!("Indexed document: {}", item.doc_id);
                        (vec![item], Vec::new())
                    }
                    Err(e) => {
                        tracing::warn!("Failed to index {}: {}", source_label, e);
                        (
                            Vec::new(),
                            vec![FailedItem::new(&source_label, e.to_string())],
                        )
                    }
                }
            }
            Ok(IndexAction::IncrementalUpdate {
                old_tree,
                existing_id,
            }) => {
                info!("Incremental update for: {}", source_label);
                match self
                    .indexer
                    .index_with_existing(source, name, options, Some(&old_tree))
                    .await
                {
                    Ok(mut doc) => {
                        doc.id = existing_id.clone();
                        let pipeline_options = self.build_pipeline_options(options, doc.format);
                        let metrics = doc.metrics.clone();
                        let item = IndexItem::new(
                            doc.id.clone(),
                            doc.name.clone(),
                            doc.format.clone(),
                            doc.description.clone(),
                            doc.page_count,
                        )
                        .with_source_path(
                            doc.source_path
                                .as_ref()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_default(),
                        )
                        .with_metrics_opt(metrics);
                        let persisted = self
                            .indexer
                            .to_persisted_with_options(doc, &pipeline_options);

                        if let Some(ref workspace) = self.workspace {
                            // save() is atomic (write-lock + put), no need to remove first
                            if let Err(e) = workspace.save(&persisted).await {
                                return (
                                    Vec::new(),
                                    vec![FailedItem::new(&source_label, e.to_string())],
                                );
                            }
                        }

                        info!("Incrementally updated: {}", item.doc_id);
                        (vec![item], Vec::new())
                    }
                    Err(e) => {
                        tracing::warn!("Incremental update failed for {}: {}", source_label, e);
                        (
                            Vec::new(),
                            vec![FailedItem::new(&source_label, e.to_string())],
                        )
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to resolve action for {}: {}", source_label, e);
                (
                    Vec::new(),
                    vec![FailedItem::new(&source_label, e.to_string())],
                )
            }
        }
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
    ///     .with_key("sk-...")
    ///     .with_model("gpt-4o")
    ///     .build()
    ///     .await?;
    ///
    /// // Single document
    /// let result = engine.query(
    ///     QueryContext::new("What is the total revenue?")
    ///         .with_doc_ids(vec!["doc-123".to_string()])
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
        let mut options = ctx.to_retrieve_options(&self.config);

        // Load document graph for graph-aware retrieval (if enabled)
        if self.config.graph.enabled {
            if let Some(ref workspace) = self.workspace {
                if let Ok(Some(graph)) = workspace.get_graph().await {
                    options = options.with_document_graph(Arc::new(graph));
                }
            }
        }

        let mut items = Vec::with_capacity(doc_ids.len());
        let mut failed = Vec::new();

        // TODO: if doc_ids.len() > 1, consider parallelizing queries across documents (with concurrency limit)
        for doc_id in doc_ids {
            let (tree, reasoning_index) = match self.get_structure(&doc_id).await {
                Ok((t, ri)) => (t, ri),
                Err(e) => {
                    tracing::warn!("Skipping document {}: {}", doc_id, e);
                    failed.push(FailedItem::new(&doc_id, e.to_string()));
                    continue;
                }
            };

            match self
                .retriever
                .query_with_reasoning_index(&tree, &ctx.query, &options, reasoning_index)
                .await
            {
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
                "Query failed for all {} document(s): {}",
                failed.len(),
                failed
                    .iter()
                    .map(|f| format!("{} ({})", f.source, f.error))
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }

        Ok(QueryResult::with_partial(items, failed))
    }

    /// Query a document with streaming results.
    ///
    /// Returns a [`RetrieveEventReceiver`] that yields [`RetrieveEvent`](crate::retrieval::RetrieveEvent)s
    /// as the retrieval pipeline progresses through each stage.
    ///
    /// Only supports single-document scope (via `with_doc_ids` with one ID).
    pub async fn query_stream(&self, ctx: QueryContext) -> Result<RetrieveEventReceiver> {
        let doc_id = match &ctx.scope {
            QueryScope::Documents(ids) if ids.len() == 1 => ids[0].clone(),
            _ => {
                return Err(Error::Config(
                    "query_stream requires a single doc_id via with_doc_ids".to_string(),
                ));
            }
        };

        let (tree, _reasoning_index) = self.get_structure(&doc_id).await?;
        let options = ctx.to_retrieve_options(&self.config);

        let rx = self
            .retriever
            .query_stream(&tree, &ctx.query, &options)
            .await?;

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

    /// Get the cross-document relationship graph.
    ///
    /// The graph is automatically rebuilt after indexing documents.
    /// Returns `None` if no graph has been built yet.
    pub async fn get_graph(&self) -> Result<Option<crate::graph::DocumentGraph>> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        workspace.get_graph().await
    }

    /// Generate a complete metrics report.
    ///
    /// Returns a [`MetricsReport`](crate::metrics::MetricsReport) containing
    /// LLM usage, pilot decision, and retrieval operation metrics.
    pub fn metrics_report(&self) -> crate::metrics::MetricsReport {
        self.metrics_hub.generate_report()
    }

    // ============================================================
    // Internal
    // ============================================================

    /// Get document structure (tree) and optional reasoning index. Internal use only.
    pub(crate) async fn get_structure(
        &self,
        doc_id: &str,
    ) -> Result<(DocumentTree, Option<crate::document::ReasoningIndex>)> {
        let workspace = self
            .workspace
            .as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        let doc = workspace
            .load(doc_id)
            .await?
            .ok_or_else(|| Error::DocumentNotFound(format!("Document not found: {}", doc_id)))?;

        Ok((doc.tree, doc.reasoning_index))
    }

    /// Resolve QueryScope into a list of document IDs.
    async fn resolve_scope(&self, scope: &QueryScope) -> Result<Vec<String>> {
        match scope {
            QueryScope::Documents(ids) => Ok(ids.clone()),
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
        format: crate::index::parse::DocumentFormat,
    ) -> PipelineOptions {
        use crate::index::SummaryStrategy;
        PipelineOptions {
            mode: match format {
                crate::index::parse::DocumentFormat::Markdown => crate::index::IndexMode::Markdown,
                crate::index::parse::DocumentFormat::Pdf => crate::index::IndexMode::Pdf,
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

    /// Rebuild the document graph after indexing, if graph is enabled.
    async fn rebuild_graph(&self) -> Result<()> {
        if !self.config.graph.enabled {
            return Ok(());
        }
        let workspace = match self.workspace {
            Some(ref ws) => ws,
            None => return Ok(()),
        };

        // Load all documents and extract keyword profiles
        let doc_ids = workspace.inner().list_documents().await;
        let mut builder = crate::graph::DocumentGraphBuilder::new(self.config.graph.clone());

        for doc_id in &doc_ids {
            if let Some(doc) = workspace.load(doc_id).await? {
                let keywords = Self::extract_keywords_from_doc(&doc);
                builder.add_document(
                    &doc.meta.id,
                    &doc.meta.name,
                    &doc.meta.format,
                    doc.meta.node_count,
                    keywords,
                );
            }
        }

        let graph = builder.build();
        workspace.set_graph(&graph).await?;
        Ok(())
    }

    /// Extract keyword → weight map from a persisted document's ReasoningIndex.
    fn extract_keywords_from_doc(doc: &PersistedDocument) -> HashMap<String, f32> {
        let mut keywords = HashMap::new();
        if let Some(ref ri) = doc.reasoning_index {
            for (kw, entries) in ri.all_topic_entries() {
                let weight: f32 =
                    entries.iter().map(|e| e.weight).sum::<f32>() / entries.len().max(1) as f32;
                keywords.insert(kw.clone(), weight);
            }
        }
        keywords
    }

    /// Resolve what action to take for a source.
    async fn resolve_index_action(
        &self,
        source: &IndexSource,
        options: &super::types::IndexOptions,
    ) -> Result<IndexAction> {
        let workspace = match self.workspace {
            Some(ref ws) => ws,
            None => return Ok(IndexAction::FullIndex { existing_id: None }),
        };

        // Force mode always re-indexes from scratch
        if options.mode == IndexMode::Force {
            return Ok(IndexAction::FullIndex { existing_id: None });
        }

        // Only path sources support incremental indexing
        let path = match source {
            IndexSource::Path(p) => p,
            _ => return Ok(IndexAction::FullIndex { existing_id: None }),
        };

        // Find if this file has already been indexed
        let existing_id = match workspace.find_by_source_path(path).await {
            Some(id) => id,
            None => return Ok(IndexAction::FullIndex { existing_id: None }), // New file
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
                format: crate::index::parse::DocumentFormat::from_extension(&format_str)
                    .unwrap_or(crate::index::parse::DocumentFormat::Markdown),
                description: desc,
                page_count: pages,
            }));
        }

        // Incremental mode: load stored document and delegate to resolver
        let current_bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => return Ok(IndexAction::FullIndex { existing_id: None }),
        };

        let stored_doc = match workspace.load(&existing_id).await? {
            Some(d) => d,
            None => return Ok(IndexAction::FullIndex { existing_id: None }),
        };

        let format = crate::index::parse::DocumentFormat::from_extension(&stored_doc.meta.format)
            .unwrap_or(crate::index::parse::DocumentFormat::Markdown);
        let pipeline_options = self.build_pipeline_options(options, format);

        // If logic fingerprint changed, remove old doc before full reprocess
        let action =
            incremental::resolve_action(&current_bytes, &stored_doc, &pipeline_options, format);

        // Note: if FullIndex, old doc cleanup happens in process_source()
        // after successful save (save-first, then remove old).

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
            metrics_hub: Arc::clone(&self.metrics_hub),
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
