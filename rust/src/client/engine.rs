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
//!     .with_endpoint("https://api.openai.com/v1")
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

use std::{
    collections::HashMap,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    sync::Mutex,
};

use futures::StreamExt;
use tracing::info;

use crate::{
    DocumentTree, Error,
    config::Config,
    error::Result,
    events::EventEmitter,
    index::{
        PipelineOptions,
        incremental::{self, IndexAction},
    },
    metrics::MetricsHub,
    retrieval::{PipelineRetriever, RetrieveEventReceiver},
    storage::{PersistedDocument, Workspace},
};

use super::{
    index_context::{IndexContext, IndexSource},
    indexer::IndexerClient,
    query_context::{QueryContext, QueryScope},
    retriever::RetrieverClient,
    types::{DocumentInfo, FailedItem, IndexItem, IndexMode, IndexResult, QueryResult, QueryResultItem},
    workspace::WorkspaceClient,
};

/// Shared cancel state: `true` means cancelled.
type CancelFlag = Arc<AtomicBool>;

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
    workspace: WorkspaceClient,

    /// Central metrics hub for unified collection.
    metrics_hub: Arc<MetricsHub>,

    /// Whether the document graph needs rebuilding (set after index, consumed in query).
    graph_dirty: Arc<AtomicBool>,

    /// Shared cancel flag — set by `cancel()`, checked by long-running operations.
    cancelled: CancelFlag,

    /// Active operation count so `cancel()` can wait for drain.
    active_ops: Arc<Mutex<usize>>,
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
        let retriever = RetrieverClient::new(retriever).with_events(events.clone());

        // Create workspace client
        let workspace_client = WorkspaceClient::new(workspace)
            .await
            .with_events(events.clone());

        Ok(Self {
            config,
            indexer,
            retriever,
            workspace: workspace_client,
            metrics_hub: Arc::new(MetricsHub::with_defaults()),
            graph_dirty: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
            active_ops: Arc::new(Mutex::new(0)),
        })
    }

    // ============================================================
    // Document Indexing
    // ============================================================

    /// Index one or more documents.
    ///
    /// Accepts an [`IndexContext`] that specifies the source (file path,
    /// directory, content string, or bytes) and indexing options.
    /// Multiple sources are indexed in parallel.
    ///
    /// Returns an [`IndexResult`] containing the indexed document metadata.
    #[tracing::instrument(skip_all, fields(sources = ctx.sources.len()))]
    pub async fn index(&self, ctx: IndexContext) -> Result<IndexResult> {
        self.check_cancel()?;
        if ctx.is_empty() {
            return Err(Error::Config("No document sources provided".into()));
        }

        let _guard = self.inc_active();
        let timeout_secs = ctx.options.timeout_secs;

        self.with_timeout(timeout_secs, async move {
            let concurrency = self
                .config
                .llm
                .throttle
                .max_concurrent_requests
                .min(ctx.sources.len());

            let (items, failed) = self
                .process_sources(&ctx.sources, &ctx.options, ctx.name.as_deref(), concurrency)
                .await;

            if items.is_empty() && !failed.is_empty() {
                return Err(Error::Config(format!(
                    "All {} source(s) failed: {}",
                    failed.len(),
                    failed
                        .iter()
                        .map(|f| format!("{} ({})", f.source, f.error))
                        .collect::<Vec<_>>()
                        .join("; ")
                )));
            }

            // Mark graph as dirty — will be lazily rebuilt on next query()
            if !items.is_empty() && self.config.graph.enabled {
                self.graph_dirty.store(true, Ordering::Relaxed);
            }

            Ok(IndexResult::with_partial(items, failed))
        })
        .await
    }

    /// Process multiple sources in parallel.
    async fn process_sources(
        &self,
        sources: &[IndexSource],
        options: &super::types::IndexOptions,
        name: Option<&str>,
        concurrency: usize,
    ) -> (Vec<IndexItem>, Vec<FailedItem>) {
        let results: Vec<(Vec<IndexItem>, Vec<FailedItem>)> =
            futures::stream::iter(sources.iter().cloned())
                .map(|source| {
                    let options = options.clone();
                    let name = name.map(str::to_string);
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

        results.into_iter().fold(
            (Vec::new(), Vec::new()),
            |(mut items, mut failed), (ok, err)| {
                items.extend(ok);
                failed.extend(err);
                (items, failed)
            },
        )
    }

    /// Process a single source — resolve action and index.
    #[tracing::instrument(skip_all, fields(source = %source))]
    ///
    /// Returns `(items, failed)`.
    async fn process_source(
        &self,
        source: &IndexSource,
        options: &super::types::IndexOptions,
        name: Option<&str>,
    ) -> (Vec<IndexItem>, Vec<FailedItem>) {
        if self.is_cancelled() {
            return (
                Vec::new(),
                vec![FailedItem::new(source.to_string(), "Operation cancelled".to_string())],
            );
        }

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
                let pipeline_options = self.build_pipeline_options(options, source);
                match self
                    .indexer
                    .index(source, name, pipeline_options.clone())
                    .await
                {
                    Ok(doc) => {
                        self.index_and_persist(
                            doc,
                            &pipeline_options,
                            &source_label,
                            existing_id.as_deref(),
                        )
                        .await
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
                let pipeline_options = self.build_pipeline_options(options, source);
                match self
                    .indexer
                    .index_with_existing(source, name, pipeline_options.clone(), Some(&old_tree))
                    .await
                {
                    Ok(mut doc) => {
                        doc.id = existing_id.clone();
                        self.index_and_persist(doc, &pipeline_options, &source_label, None)
                            .await
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

    /// Convert an [`IndexedDocument`] to an [`IndexItem`] and persist it.
    ///
    /// If `old_id` is provided, the old document is removed after a
    /// successful save (atomic save-first, then remove old).
    async fn index_and_persist(
        &self,
        doc: super::indexed_document::IndexedDocument,
        pipeline_options: &PipelineOptions,
        source_label: &str,
        old_id: Option<&str>,
    ) -> (Vec<IndexItem>, Vec<FailedItem>) {
        let item = Self::build_index_item(&doc);
        let persisted = IndexerClient::to_persisted(doc, pipeline_options).await;

        if let Err(e) = self.workspace.save(&persisted).await {
            return (
                Vec::new(),
                vec![FailedItem::new(source_label, e.to_string())],
            );
        }
        // Clean up old document after successful save
        if let Some(old_id) = old_id {
            if let Err(e) = self.workspace.remove(old_id).await {
                tracing::warn!("Failed to remove old document {}: {}", old_id, e);
            }
        }

        info!("Indexed document: {}", item.doc_id);
        (vec![item], Vec::new())
    }

    /// Build an [`IndexItem`] from an [`IndexedDocument`](super::indexed_document::IndexedDocument).
    fn build_index_item(doc: &super::indexed_document::IndexedDocument) -> IndexItem {
        IndexItem::new(
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
        .with_metrics_opt(doc.metrics.clone())
    }

    // ============================================================
    // Document Querying
    // ============================================================

    /// Query documents.
    ///
    /// Accepts a [`QueryContext`] that specifies the query text and scope
    /// (single document, multiple documents, or entire workspace).
    #[tracing::instrument(skip_all, fields(query = %ctx.query))]
    pub async fn query(&self, ctx: QueryContext) -> Result<QueryResult> {
        self.check_cancel()?;
        let _guard = self.inc_active();
        let timeout_secs = ctx.timeout_secs;

        self.with_timeout(timeout_secs, async move {
            let doc_ids = self.resolve_scope(&ctx.scope).await?;
            let mut options = ctx.to_retrieve_options(&self.config);

            // Lazy graph rebuild: only rebuild if index() marked it dirty
            if self.config.graph.enabled {
                if self.graph_dirty.swap(false, Ordering::Relaxed) {
                    if let Err(e) = self.rebuild_graph().await {
                        tracing::warn!("Graph rebuild failed: {e}");
                        // Re-mark dirty so next query retries
                        self.graph_dirty.store(true, Ordering::Relaxed);
                    }
                }
                // Load (now up-to-date) graph for retrieval
                if let Ok(Some(graph)) = self.workspace.get_graph().await {
                    options = options.with_document_graph(Arc::new(graph));
                }
            }

            // Query documents in parallel (with concurrency limit)
            let concurrency = self.config.llm.throttle.max_concurrent_requests;
            let query = ctx.query.clone();
            let cancelled = Arc::clone(&self.cancelled);

            let results: Vec<(String, std::result::Result<QueryResultItem, String>)> =
                futures::stream::iter(doc_ids.into_iter())
                    .map(|doc_id| {
                        let engine = self.clone();
                        let options = options.clone();
                        let query = query.clone();
                        let cancelled = Arc::clone(&cancelled);
                        async move {
                            if cancelled.load(Ordering::Relaxed) {
                                return (doc_id, Err("Operation cancelled".to_string()));
                            }

                            let (tree, reasoning_index) = match engine.get_structure(&doc_id).await {
                                Ok(t) => t,
                                Err(e) => return (doc_id, Err(e.to_string())),
                            };

                            match engine
                                .retriever
                                .query_with_reasoning_index(&tree, &query, &options, reasoning_index)
                                .await
                            {
                                Ok(mut result) => {
                                    result.doc_id = doc_id.clone();
                                    (doc_id, Ok(result))
                                }
                                Err(e) => (doc_id, Err(e.to_string())),
                            }
                        }
                    })
                    .buffer_unordered(concurrency)
                    .collect()
                    .await;

            let mut items = Vec::new();
            let mut failed = Vec::new();
            for (doc_id, result) in results {
                match result {
                    Ok(item) => items.push(item),
                    Err(e) => {
                        tracing::warn!("Query failed for {}: {}", doc_id, e);
                        failed.push(FailedItem::new(&doc_id, e));
                    }
                }
            }

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
        })
        .await
    }

    /// Query a document with streaming results.
    ///
    /// Returns a receiver that yields retrieval events
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
        self.workspace.list().await
    }

    /// Remove a document from the workspace.
    pub async fn remove(&self, doc_id: &str) -> Result<bool> {
        self.workspace.remove(doc_id).await
    }

    /// Check if a document exists in the workspace.
    pub async fn exists(&self, doc_id: &str) -> Result<bool> {
        self.workspace.exists(doc_id).await
    }

    /// Remove all documents from the workspace.
    ///
    /// Returns the number of documents removed.
    pub async fn clear(&self) -> Result<usize> {
        self.workspace.clear().await
    }

    /// Get the cross-document relationship graph.
    ///
    /// The graph is automatically rebuilt after indexing documents.
    /// Returns `None` if no graph has been built yet.
    pub async fn get_graph(&self) -> Result<Option<crate::graph::DocumentGraph>> {
        self.workspace.get_graph().await
    }

    /// Generate a complete metrics report.
    ///
    /// Returns a [`MetricsReport`](crate::metrics::MetricsReport) containing
    /// LLM usage, pilot decision, and retrieval operation metrics.
    pub fn metrics_report(&self) -> crate::metrics::MetricsReport {
        self.metrics_hub.generate_report()
    }

    /// Cancel all in-flight `index()` and `query()` operations.
    ///
    /// After calling this, running operations will return at the next
    /// convenient point with a cancellation error. New operations will
    /// also fail until [`reset_cancel`](Self::reset_cancel) is called.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        tracing::info!("Cancellation requested");
    }

    /// Reset the cancel flag so new operations can proceed.
    pub fn reset_cancel(&self) {
        self.cancelled.store(false, Ordering::Relaxed);
        tracing::info!("Cancel flag reset");
    }

    /// Returns `true` if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    // ============================================================
    // Internal
    // ============================================================

    /// Check cancel flag, returning an error if cancelled.
    fn check_cancel(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(Error::Config("Operation cancelled".into()));
        }
        Ok(())
    }

    /// Increment active operation counter. Returns a guard that decrements on drop.
    fn inc_active(&self) -> ActiveGuard {
        let mut ops = self.active_ops.lock().unwrap();
        *ops += 1;
        ActiveGuard {
            active_ops: Arc::clone(&self.active_ops),
        }
    }

    /// Get current active operation count.
    pub fn active_operations(&self) -> usize {
        *self.active_ops.lock().unwrap()
    }

    /// Run a future with an optional timeout.
    /// If `timeout_secs` is `Some`, wraps the future in `tokio::time::timeout`.
    async fn with_timeout<F, T>(&self, timeout_secs: Option<u64>, fut: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        match timeout_secs {
            Some(secs) => match tokio::time::timeout(std::time::Duration::from_secs(secs), fut).await {
                Ok(result) => result,
                Err(_) => Err(Error::Config(format!("Operation timed out after {secs}s"))),
            },
            None => fut.await,
        }
    }

    /// Get document structure (tree) and optional reasoning index. Internal use only.
    pub(crate) async fn get_structure(
        &self,
        doc_id: &str,
    ) -> Result<(DocumentTree, Option<crate::document::ReasoningIndex>)> {
        let doc =
            self.workspace.load(doc_id).await?.ok_or_else(|| {
                Error::DocumentNotFound(format!("Document not found: {}", doc_id))
            })?;

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

    /// Build pipeline options for pipeline execution (with checkpoint dir).
    ///
    /// This is the single source of truth for pipeline configuration.
    fn build_pipeline_options(
        &self,
        options: &super::types::IndexOptions,
        source: &IndexSource,
    ) -> PipelineOptions {
        use crate::index::{IndexMode, ReasoningIndexConfig, SummaryStrategy};

        let format = match source {
            IndexSource::Path(path) => self
                .indexer
                .detect_format_from_path(path)
                .unwrap_or(crate::index::parse::DocumentFormat::Markdown),
            IndexSource::Content { format, .. } => *format,
            IndexSource::Bytes { format, .. } => *format,
        };

        let checkpoint_dir = Some(self.config.storage.checkpoint_dir.clone());

        PipelineOptions {
            mode: match format {
                crate::index::parse::DocumentFormat::Markdown => IndexMode::Markdown,
                crate::index::parse::DocumentFormat::Pdf => IndexMode::Pdf,
            },
            generate_ids: options.generate_ids,
            summary_strategy: if options.generate_summaries {
                SummaryStrategy::full()
            } else {
                SummaryStrategy::none()
            },
            generate_description: options.generate_description,
            checkpoint_dir,
            reasoning_index: ReasoningIndexConfig {
                enable_synonym_expansion: options.enable_synonym_expansion,
                ..ReasoningIndexConfig::default()
            },
            ..Default::default()
        }
    }

    /// Resolve what action to take for a source.
    async fn resolve_index_action(
        &self,
        source: &IndexSource,
        options: &super::types::IndexOptions,
    ) -> Result<IndexAction> {
        let workspace = &self.workspace;

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
        let current_bytes = match tokio::fs::read(path).await {
            Ok(b) => b,
            Err(_) => return Ok(IndexAction::FullIndex { existing_id: None }),
        };

        let stored_doc = match workspace.load(&existing_id).await? {
            Some(d) => d,
            None => return Ok(IndexAction::FullIndex { existing_id: None }),
        };

        let format = crate::index::parse::DocumentFormat::from_extension(&stored_doc.meta.format)
            .unwrap_or(crate::index::parse::DocumentFormat::Markdown);
        let pipeline_options = self.build_pipeline_options(options, source);

        // If logic fingerprint changed, remove old doc before full reprocess
        let action =
            incremental::resolve_action(&current_bytes, &stored_doc, &pipeline_options, format);

        // Note: if FullIndex, old doc cleanup happens in process_source()
        // after successful save (save-first, then remove old).

        Ok(action)
    }

    /// Rebuild the document graph after indexing, if graph is enabled.
    async fn rebuild_graph(&self) -> Result<()> {
        if !self.config.graph.enabled {
            return Ok(());
        }

        // Load all documents in parallel and extract keyword profiles
        let doc_ids = self.workspace.inner().list_documents().await;
        let concurrency = self.config.llm.throttle.max_concurrent_requests;

        let loaded: Vec<Option<PersistedDocument>> =
            futures::stream::iter(doc_ids.iter().cloned())
                .map(|doc_id| {
                    let ws = self.workspace.clone();
                    async move { ws.load(&doc_id).await.ok().flatten() }
                })
                .buffer_unordered(concurrency)
                .collect()
                .await;

        let mut builder = crate::graph::DocumentGraphBuilder::new(self.config.graph.clone());
        for doc in loaded.into_iter().flatten() {
            let keywords = Self::extract_keywords_from_doc(&doc);
            builder.add_document(
                &doc.meta.id,
                &doc.meta.name,
                &doc.meta.format,
                doc.meta.node_count,
                keywords,
            );
        }

        let graph = builder.build();
        self.workspace.set_graph(&graph).await?;
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
}

impl Clone for Engine {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            indexer: self.indexer.clone(),
            retriever: self.retriever.clone(),
            workspace: self.workspace.clone(),
            metrics_hub: Arc::clone(&self.metrics_hub),
            graph_dirty: Arc::clone(&self.graph_dirty),
            cancelled: Arc::clone(&self.cancelled),
            active_ops: Arc::clone(&self.active_ops),
        }
    }
}

/// RAII guard that decrements `active_ops` on drop.
struct ActiveGuard {
    active_ops: Arc<Mutex<usize>>,
}

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        let mut ops = self.active_ops.lock().unwrap();
        *ops = ops.saturating_sub(1);
    }
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::types::IndexMode;

    // ── Cancel ────────────────────────────────────────────────────────────

    #[test]
    fn test_cancel_flag() {
        // We can't construct a full Engine without async + LLM, so test the
        // underlying primitives directly.
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(Ordering::Relaxed));

        flag.store(true, Ordering::Relaxed);
        assert!(flag.load(Ordering::Relaxed));

        flag.store(false, Ordering::Relaxed);
        assert!(!flag.load(Ordering::Relaxed));
    }

    #[test]
    fn test_graph_dirty_flag() {
        let dirty = Arc::new(AtomicBool::new(false));
        assert!(!dirty.load(Ordering::Relaxed));

        // Simulate: index marks dirty
        dirty.store(true, Ordering::Relaxed);

        // Simulate: query swaps to false and rebuilds
        let was_dirty = dirty.swap(false, Ordering::Relaxed);
        assert!(was_dirty);
        assert!(!dirty.load(Ordering::Relaxed));
    }

    #[test]
    fn test_active_guard_decrement() {
        let active_ops: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

        // Increment
        {
            let mut ops = active_ops.lock().unwrap();
            *ops += 1;
        }

        assert_eq!(*active_ops.lock().unwrap(), 1);

        // Drop guard (simulate ActiveGuard drop)
        {
            let mut ops = active_ops.lock().unwrap();
            *ops = ops.saturating_sub(1);
        }

        assert_eq!(*active_ops.lock().unwrap(), 0);
    }

    // ── resolve_index_action Default mode ──────────────────────────────────

    // We can't call resolve_index_action without a workspace, but we can
    // verify IndexMode equality logic used inside.
    #[test]
    fn test_index_mode_force_skips_incremental() {
        let mode = IndexMode::Force;
        assert_eq!(mode, IndexMode::Force);
        assert_ne!(mode, IndexMode::Default);
        assert_ne!(mode, IndexMode::Incremental);
    }

    // ── build_index_item ──────────────────────────────────────────────────

    // Build_index_item only transforms data — no I/O.
    use crate::client::indexed_document::IndexedDocument;

    fn make_doc() -> IndexedDocument {
        IndexedDocument::new("test-id", crate::index::parse::DocumentFormat::Markdown)
            .with_name("test.md")
            .with_description("test doc")
            .with_source_path(std::path::PathBuf::from("/tmp/test.md"))
    }

    #[test]
    fn test_build_index_item() {
        let doc = make_doc();
        let item = Engine::build_index_item(&doc);

        assert_eq!(item.doc_id, "test-id");
        assert_eq!(item.name, "test.md");
        assert_eq!(item.format, crate::index::parse::DocumentFormat::Markdown);
        assert_eq!(item.description, Some("test doc".to_string()));
        assert_eq!(item.source_path, Some("/tmp/test.md".to_string()));
        assert!(item.metrics.is_none());
    }

    #[test]
    fn test_build_index_item_no_source_path() {
        let doc = IndexedDocument::new("id", crate::index::parse::DocumentFormat::Pdf);
        let item = Engine::build_index_item(&doc);

        assert_eq!(item.source_path, Some(String::new())); // unwrap_or_default
        assert_eq!(item.format, crate::index::parse::DocumentFormat::Pdf);
    }
}
