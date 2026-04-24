// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Main Engine client - the entry point for vectorless.
//!
//! The Engine provides a unified API for the Document Understanding Engine:
//!
//! - [`compile`](Engine::compile) — Understand a document (parse, analyze, persist)
//! - [`forget`](Engine::forget) — Remove a document
//! - [`list_documents`](Engine::list_documents) — List all understood documents

use std::{collections::HashMap, sync::Arc};

use futures::StreamExt;
use tracing::{info, warn};

use vectorless_config::Config;
use vectorless_document::{
    Document as UnderstandingDocument, DocumentTree, IngestInput,
};
use vectorless_error::{Error, Result};
use vectorless_events::EventEmitter;
use vectorless_compiler::{
    PipelineOptions,
    incremental::{self, IndexAction},
};
use vectorless_metrics::MetricsHub;
use vectorless_storage::{PersistedDocument, Workspace};

use super::{
    compile_input::{CompileInput, CompileSource},
    indexer::IndexerClient,
    types::{FailedItem, CompileArtifact, CompileMode, CompileOutput},
    workspace::WorkspaceClient,
};

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

    /// Workspace client for persistence.
    workspace: WorkspaceClient,

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
        indexer: IndexerClient,
        events: EventEmitter,
        metrics_hub: Arc<MetricsHub>,
    ) -> Result<Self> {
        let config = Arc::new(config);

        // Attach event emitter to indexer
        let indexer = indexer.with_events(events.clone());

        // Create workspace client
        let workspace_client = WorkspaceClient::new(workspace)
            .await
            .with_events(events.clone());

        Ok(Self {
            config,
            indexer,
            workspace: workspace_client,
            metrics_hub,
        })
    }

    // ============================================================
    // Compile Pipeline (private — called by compile())
    // ============================================================

    /// Run the compile pipeline: parse, compile, persist.
    ///
    /// Accepts an [`CompileInput`] that specifies the source and options.
    /// Multiple sources are processed in parallel.
    /// Returns an [`CompileOutput`] containing the indexed document metadata.
    #[tracing::instrument(skip_all, fields(sources = ctx.sources.len()))]
    async fn compile_pipeline(&self, ctx: CompileInput) -> Result<CompileOutput> {
        if ctx.is_empty() {
            return Err(Error::Config("No document sources provided".into()));
        }

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

            // Rebuild cross-document graph in the background so index returns immediately.
            if !items.is_empty() && self.config.graph.enabled {
                let engine = self.clone();
                tokio::spawn(async move {
                    info!("Rebuilding document graph in background...");
                    if let Err(e) = engine.rebuild_graph().await {
                        tracing::warn!("Background graph rebuild failed: {e}");
                    }
                });
            }

            Ok(CompileOutput::with_partial(items, failed))
        })
        .await
    }

    /// Process multiple sources in parallel.
    async fn process_sources(
        &self,
        sources: &[CompileSource],
        options: &super::types::CompileOptions,
        name: Option<&str>,
        concurrency: usize,
    ) -> (Vec<CompileArtifact>, Vec<FailedItem>) {
        let results: Vec<(Vec<CompileArtifact>, Vec<FailedItem>)> =
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
        source: &CompileSource,
        options: &super::types::CompileOptions,
        name: Option<&str>,
    ) -> (Vec<CompileArtifact>, Vec<FailedItem>) {
        let source_label = source.to_string();

        match self.resolve_index_action(source, options).await {
            Ok(IndexAction::Skip(skip_info)) => {
                info!("Skipped (unchanged): {}", source_label);
                (
                    vec![CompileArtifact::new(
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
                    .index_with_retry(source, name, pipeline_options.clone(), None)
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
                    .index_with_retry(source, name, pipeline_options.clone(), Some(&old_tree))
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

    /// Index with retry on retryable errors.
    ///
    /// Reads `config.llm.retry` for backoff parameters.
    /// Returns `Err` only after all retries are exhausted or the error
    /// is not retryable.
    async fn index_with_retry(
        &self,
        source: &CompileSource,
        name: Option<&str>,
        pipeline_options: PipelineOptions,
        existing_tree: Option<&DocumentTree>,
    ) -> Result<super::indexed_document::IndexedDocument> {
        let retry = &self.config.llm.retry;
        let max_attempts = retry.max_attempts;

        for attempt in 0..max_attempts {
            let result = if let Some(tree) = existing_tree {
                self.indexer
                    .index_with_existing(source, name, pipeline_options.clone(), Some(tree))
                    .await
            } else {
                self.indexer
                    .index(source, name, pipeline_options.clone())
                    .await
            };

            match result {
                Ok(doc) => return Ok(doc),
                Err(e) if e.is_retryable() && attempt + 1 < max_attempts => {
                    let delay = retry.delay_for_attempt(attempt);
                    tracing::warn!(
                        attempt,
                        max_attempts,
                        ?delay,
                        "Retryable error indexing, retrying: {e}"
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(e) => return Err(e),
            }
        }

        // Unreachable: loop always returns via Ok/Err branches
        unreachable!()
    }

    /// Convert an [`IndexedDocument`] to an [`CompileArtifact`] and persist it.
    ///
    /// If `old_id` is provided, the old document is removed after a
    /// successful save (atomic save-first, then remove old).
    async fn index_and_persist(
        &self,
        doc: super::indexed_document::IndexedDocument,
        pipeline_options: &PipelineOptions,
        source_label: &str,
        old_id: Option<&str>,
    ) -> (Vec<CompileArtifact>, Vec<FailedItem>) {
        let item = Self::build_index_item(&doc);

        info!("[index] Persisting document '{}'...", doc.name,);
        let persisted = IndexerClient::to_persisted(doc, pipeline_options).await;

        if let Err(e) = self.workspace.save(&persisted).await {
            warn!("[index] Failed to save document: {}", e);
            return (
                Vec::new(),
                vec![FailedItem::new(source_label, e.to_string())],
            );
        }
        // Clean up old document after successful save
        if let Some(old_id) = old_id {
            if let Err(e) = self.workspace.remove(old_id).await {
                warn!("Failed to remove old document {}: {}", old_id, e);
            }
        }

        info!("[index] Document persisted: {}", item.doc_id);
        (vec![item], Vec::new())
    }

    /// Build an [`CompileArtifact`] from an [`IndexedDocument`](super::indexed_document::IndexedDocument).
    fn build_index_item(doc: &super::indexed_document::IndexedDocument) -> CompileArtifact {
        CompileArtifact::new(
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
    // Understanding Engine API
    // ============================================================

    /// Understand a document — parse, analyze, and persist.
    ///
    /// Returns a [`vectorless_document::DocumentInfo`] with summary, structure, and concepts.
    /// The engine builds a full understanding including tree, navigation index,
    /// reasoning index, summary, and key concepts.
    pub async fn compile(&self, input: IngestInput) -> Result<vectorless_document::DocumentInfo> {
        let ctx = match &input {
            IngestInput::Path(path) => CompileInput::from_path(path),
            IngestInput::Bytes { data, format, .. } => {
                CompileInput::from_bytes(data.clone(), *format)
            }
            IngestInput::Text { content, .. } => CompileInput::from_content(
                content,
                vectorless_compiler::parse::DocumentFormat::Markdown,
            ),
        };

        let result = self.compile_pipeline(ctx).await?;

        let doc_id = result
            .doc_id()
            .ok_or_else(|| Error::Config("compile produced no results".into()))?
            .to_string();

        // Load the persisted document to build DocumentInfo
        let persisted = self
            .workspace
            .load(&doc_id)
            .await?
            .ok_or_else(|| Error::Config("Document not found after compile".into()))?;

        let doc = Self::persisted_to_understanding_document(persisted);
        Ok(doc.info())
    }

    /// Remove a document from the workspace.
    pub async fn forget(&self, doc_id: &str) -> Result<()> {
        self.workspace.remove(doc_id).await?;
        Ok(())
    }

    /// List all understood documents.
    ///
    /// Returns [`Vec<vectorless_document::DocumentInfo>`] with summary, structure, and concepts
    /// for each document.
    pub async fn list_documents(&self) -> Result<Vec<vectorless_document::DocumentInfo>> {
        let ids = self.workspace.inner().list_documents().await;
        let mut result = Vec::new();
        for id in ids {
            match self.workspace.load(&id).await {
                Ok(Some(persisted)) => {
                    result.push(Self::persisted_to_understanding_document(persisted).info());
                }
                Ok(None) => {
                    tracing::warn!(doc_id = %id, "Document in index but not in storage");
                }
                Err(e) => {
                    tracing::warn!(doc_id = %id, error = %e, "Failed to load document");
                }
            }
        }
        Ok(result)
    }

    // ============================================================
    // Utility Methods
    // ============================================================

    /// Check if a document exists in the workspace.
    pub async fn exists(&self, doc_id: &str) -> Result<bool> {
        self.workspace.exists(doc_id).await
    }

    /// Load a full Document by ID (for navigation via primitives).
    pub async fn load_document(
        &self,
        doc_id: &str,
    ) -> Result<Option<vectorless_document::Document>> {
        match self.workspace.load(doc_id).await? {
            Some(persisted) => Ok(Some(Self::persisted_to_understanding_document(persisted))),
            None => Ok(None),
        }
    }

    /// List all document IDs in the workspace.
    pub async fn list_document_ids(&self) -> Result<Vec<String>> {
        Ok(self.workspace.inner().list_documents().await)
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
    pub async fn get_graph(&self) -> Result<Option<vectorless_graph::DocumentGraph>> {
        self.workspace.get_graph().await
    }

    /// Generate a complete metrics report.
    ///
    /// Returns a [`MetricsReport`](vectorless_metrics::MetricsReport) containing
    /// LLM usage and retrieval operation metrics.
    pub fn metrics_report(&self) -> vectorless_metrics::MetricsReport {
        self.metrics_hub.generate_report()
    }

    // ============================================================
    // Internal: type conversions
    // ============================================================

    /// Convert a PersistedDocument to a Document (understanding type).
    fn persisted_to_understanding_document(persisted: PersistedDocument) -> UnderstandingDocument {
        let nav_index = persisted.navigation_index.unwrap_or_default();
        let reasoning_index = persisted.reasoning_index.unwrap_or_default();
        let tree = persisted.tree;

        let section_count = tree.node_count();

        UnderstandingDocument {
            doc_id: persisted.meta.id,
            name: persisted.meta.name,
            format: persisted.meta.format,
            source_path: persisted
                .meta
                .source_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            tree,
            nav_index,
            reasoning_index,
            summary: persisted.meta.description.unwrap_or_default(),
            concepts: persisted.concepts,
            page_count: persisted.meta.page_count,
            section_count,
        }
    }

    // ============================================================
    // Internal
    // ============================================================

    /// Run a future with an optional timeout.
    /// If `timeout_secs` is `Some`, wraps the future in `tokio::time::timeout`.
    async fn with_timeout<F, T>(&self, timeout_secs: Option<u64>, fut: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        match timeout_secs {
            Some(secs) => {
                match tokio::time::timeout(std::time::Duration::from_secs(secs), fut).await {
                    Ok(result) => result,
                    Err(_) => Err(Error::Config(format!("Operation timed out after {secs}s"))),
                }
            }
            None => fut.await,
        }
    }

    /// Build pipeline options for pipeline execution (with checkpoint dir).
    ///
    /// This is the single source of truth for pipeline configuration.
    fn build_pipeline_options(
        &self,
        options: &super::types::CompileOptions,
        source: &CompileSource,
    ) -> PipelineOptions {
        use vectorless_compiler::{SourceFormat, ReasoningIndexConfig, SummaryStrategy};

        let format = match source {
            CompileSource::Path(path) => self
                .indexer
                .detect_format_from_path(path)
                .unwrap_or(vectorless_compiler::parse::DocumentFormat::Markdown),
            CompileSource::Content { format, .. } => *format,
            CompileSource::Bytes { format, .. } => *format,
        };

        let checkpoint_dir = Some(self.config.storage.checkpoint_dir.clone());

        PipelineOptions {
            mode: match format {
                vectorless_compiler::parse::DocumentFormat::Markdown => SourceFormat::Markdown,
                vectorless_compiler::parse::DocumentFormat::Pdf => SourceFormat::Pdf,
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
            concurrency: vectorless_llm::throttle::ConcurrencyConfig::from(
                &self.config.llm.throttle,
            ),
            ..Default::default()
        }
    }

    /// Resolve what action to take for a source.
    async fn resolve_index_action(
        &self,
        source: &CompileSource,
        options: &super::types::CompileOptions,
    ) -> Result<IndexAction> {
        let workspace = &self.workspace;

        // Force mode always re-indexes from scratch
        if options.mode == CompileMode::Force {
            return Ok(IndexAction::FullIndex { existing_id: None });
        }

        // Only path sources support incremental indexing
        let path = match source {
            CompileSource::Path(p) => p,
            _ => return Ok(IndexAction::FullIndex { existing_id: None }),
        };

        // Find if this file has already been indexed
        let existing_id = match workspace.find_by_source_path(path).await {
            Some(id) => id,
            None => return Ok(IndexAction::FullIndex { existing_id: None }), // New file
        };

        // Default mode: skip if already indexed (no content check)
        if options.mode == CompileMode::Default {
            let info = workspace.get_document_info(&existing_id).await?;
            let (name, format_str, desc, pages) = match info {
                Some(i) => (i.name, i.format, i.description, i.page_count),
                None => (String::new(), String::new(), None, None),
            };
            return Ok(IndexAction::Skip(incremental::SkipInfo {
                doc_id: existing_id,
                name,
                format: vectorless_compiler::parse::DocumentFormat::from_extension(&format_str)
                    .unwrap_or(vectorless_compiler::parse::DocumentFormat::Markdown),
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

        let format =
            vectorless_compiler::parse::DocumentFormat::from_extension(&stored_doc.meta.format)
                .unwrap_or(vectorless_compiler::parse::DocumentFormat::Markdown);
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
        info!(
            doc_count = doc_ids.len(),
            "Loading documents for graph rebuild"
        );
        let concurrency = self.config.llm.throttle.max_concurrent_requests;

        let doc_ids_clone: Vec<String> = doc_ids.iter().cloned().collect();
        let loaded: Vec<(String, Result<Option<PersistedDocument>>)> =
            futures::stream::iter(doc_ids_clone.into_iter())
                .map(|doc_id| {
                    let ws = self.workspace.clone();
                    async move {
                        let result = ws.load(&doc_id).await;
                        (doc_id, result)
                    }
                })
                .buffer_unordered(concurrency)
                .collect()
                .await;

        let mut failed_count = 0usize;
        let mut loaded_docs: Vec<PersistedDocument> = Vec::new();
        for (doc_id, result) in loaded {
            match result {
                Ok(Some(doc)) => loaded_docs.push(doc),
                Ok(None) => {
                    warn!(
                        doc_id,
                        "Document in meta index but not in backend during graph rebuild"
                    );
                    failed_count += 1;
                }
                Err(e) => {
                    warn!(doc_id, error = %e, "Failed to load document for graph rebuild");
                    failed_count += 1;
                }
            }
        }

        info!(
            loaded = loaded_docs.len(),
            failed = failed_count,
            "Documents loaded for graph rebuild"
        );

        let mut builder = vectorless_graph::DocumentGraphBuilder::new(self.config.graph.clone());
        for doc in &loaded_docs {
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
        info!(
            nodes = graph.node_count(),
            edges = graph.edge_count(),
            "Graph built, persisting"
        );
        self.workspace.set_graph(&graph).await?;
        Ok(())
    }

    /// Extract keyword -> weight map from a persisted document's ReasoningIndex.
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
            workspace: self.workspace.clone(),
            metrics_hub: Arc::clone(&self.metrics_hub),
        }
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
    use crate::types::CompileMode;

    // -- resolve_index_action Default mode ----------------------------------------------

    // We can't call resolve_index_action without a workspace, but we can
    // verify CompileMode equality logic used inside.
    #[test]
    fn test_index_mode_force_skips_incremental() {
        let mode = CompileMode::Force;
        assert_eq!(mode, CompileMode::Force);
        assert_ne!(mode, CompileMode::Default);
        assert_ne!(mode, CompileMode::Incremental);
    }

    // -- build_index_item ----------------------------------------------------------------

    // Build_index_item only transforms data -- no I/O.
    use crate::indexed_document::IndexedDocument;

    fn make_doc() -> IndexedDocument {
        IndexedDocument::new("test-id", vectorless_compiler::parse::DocumentFormat::Markdown)
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
        assert_eq!(
            item.format,
            vectorless_compiler::parse::DocumentFormat::Markdown
        );
        assert_eq!(item.description, Some("test doc".to_string()));
        assert_eq!(item.source_path, Some("/tmp/test.md".to_string()));
        assert!(item.metrics.is_none());
    }

    #[test]
    fn test_build_index_item_no_source_path() {
        let doc = IndexedDocument::new("id", vectorless_compiler::parse::DocumentFormat::Pdf);
        let item = Engine::build_index_item(&doc);

        assert_eq!(item.source_path, Some(String::new())); // unwrap_or_default
        assert_eq!(item.format, vectorless_compiler::parse::DocumentFormat::Pdf);
    }
}
