// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document indexing client.
//!
//! This module provides document indexing operations including
//! format detection, parsing, and tree building.
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::client::{IndexerClient, IndexContext};
//!
//! let indexer = IndexerClient::new(executor);
//!
//! let result = indexer
//!     .index(IndexContext::from_path("./document.md"))
//!     .await?;
//!
//! println!("Indexed: {} ({} nodes)", result.id, result.tree.as_ref().map(|t| t.node_count()).unwrap_or(0));
//! ```

use std::path::Path;
use std::sync::Arc;

use tracing::info;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::index::parse::DocumentFormat;
use crate::index::{IndexInput, PipelineExecutor, PipelineOptions};
use crate::llm::LlmClient;
use crate::storage::{DocumentMeta, PersistedDocument};

use super::index_context::IndexSource;
use super::types::IndexedDocument;
use crate::events::{EventEmitter, IndexEvent};

/// Document indexing client.
///
/// Provides operations for parsing and indexing documents.
/// Each index operation creates a fresh pipeline executor, enabling
/// true parallel document indexing without mutex contention.
pub(crate) struct IndexerClient {
    /// Factory for creating pipeline executors (one per index operation).
    executor_factory: Arc<dyn Fn() -> PipelineExecutor + Send + Sync>,

    /// Event emitter.
    events: EventEmitter,
}

impl IndexerClient {
    /// Create with an LLM-enabled pipeline.
    pub fn with_llm(client: LlmClient) -> Self {
        let client = Arc::new(client);
        Self {
            executor_factory: Arc::new(move || PipelineExecutor::with_llm((*client).clone())),
            events: EventEmitter::new(),
        }
    }

    /// Create with event emitter.
    pub fn with_events(mut self, events: EventEmitter) -> Self {
        self.events = events;
        self
    }

    /// Index a document from an index context.
    ///
    /// The caller provides fully constructed [`PipelineOptions`]
    /// (including checkpoint dir, reasoning config, etc.).
    pub async fn index(
        &self,
        source: &IndexSource,
        name: Option<&str>,
        pipeline_options: PipelineOptions,
    ) -> Result<IndexedDocument> {
        self.index_with_existing(source, name, pipeline_options, None)
            .await
    }

    /// Index a document, optionally reusing an existing tree for incremental updates.
    ///
    /// The caller provides fully constructed [`PipelineOptions`].
    pub async fn index_with_existing(
        &self,
        source: &IndexSource,
        name: Option<&str>,
        mut pipeline_options: PipelineOptions,
        existing_tree: Option<&crate::DocumentTree>,
    ) -> Result<IndexedDocument> {
        pipeline_options.existing_tree = existing_tree.cloned();
        match source {
            IndexSource::Path(path) => {
                self.index_from_path(path, name, pipeline_options).await
            }
            IndexSource::Content { data, format } => {
                self.index_from_content(data, *format, name, pipeline_options)
                    .await
            }
            IndexSource::Bytes { data, format } => {
                self.index_from_bytes(data, *format, name, pipeline_options)
                    .await
            }
        }
    }

    /// Index from a file path.
    async fn index_from_path(
        &self,
        path: &Path,
        name: Option<&str>,
        pipeline_options: PipelineOptions,
    ) -> Result<IndexedDocument> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Validate file before indexing
        let validation = crate::utils::validate_file(&path)?;
        if !validation.valid {
            return Err(Error::Parse(
                validation
                    .errors
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Invalid file".to_string()),
            ));
        }
        for warning in &validation.warnings {
            tracing::warn!("{}", warning);
        }

        // Emit start event
        self.events.emit_index(IndexEvent::Started {
            path: path.display().to_string(),
        });

        // Generate document ID
        let doc_id = Uuid::new_v4().to_string();

        // Detect format from extension
        let format = self.detect_format_from_path(&path)?;
        self.events
            .emit_index(IndexEvent::FormatDetected { format });

        info!("Indexing {:?} document: {}", format, path.display());

        // Create pipeline input and execute
        let input = IndexInput::file(&path);
        let mut executor = (self.executor_factory)();
        let result = executor.execute(input, pipeline_options).await?;

        self.build_indexed_document(doc_id, result, format, name, Some(&path))
    }

    /// Index from content string.
    async fn index_from_content(
        &self,
        content: &str,
        format: DocumentFormat,
        name: Option<&str>,
        pipeline_options: PipelineOptions,
    ) -> Result<IndexedDocument> {
        // Validate content before indexing
        let validation = crate::utils::validate_content(content, format);
        if !validation.valid {
            return Err(Error::Parse(
                validation
                    .errors
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Invalid content".to_string()),
            ));
        }

        self.events.emit_index(IndexEvent::Started {
            path: name.unwrap_or("content").to_string(),
        });

        let doc_id = Uuid::new_v4().to_string();
        self.events
            .emit_index(IndexEvent::FormatDetected { format });

        info!("Indexing {:?} document from content", format);

        let input = IndexInput::content(content);
        let mut executor = (self.executor_factory)();
        let result = executor.execute(input, pipeline_options).await?;

        self.build_indexed_document(doc_id, result, format, name, None)
    }

    /// Index from binary data.
    async fn index_from_bytes(
        &self,
        bytes: &[u8],
        format: DocumentFormat,
        name: Option<&str>,
        pipeline_options: PipelineOptions,
    ) -> Result<IndexedDocument> {
        // Validate bytes before indexing
        let validation = crate::utils::validate_bytes(bytes, format);
        if !validation.valid {
            return Err(Error::Parse(
                validation
                    .errors
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Invalid bytes".to_string()),
            ));
        }

        self.events.emit_index(IndexEvent::Started {
            path: name.unwrap_or("bytes").to_string(),
        });

        let doc_id = Uuid::new_v4().to_string();
        self.events
            .emit_index(IndexEvent::FormatDetected { format });

        info!(
            "Indexing {:?} document from bytes ({} bytes)",
            format,
            bytes.len()
        );

        let input = IndexInput::bytes(bytes);
        let mut executor = (self.executor_factory)();
        let result = executor.execute(input, pipeline_options).await?;

        self.build_indexed_document(doc_id, result, format, name, None)
    }

    /// Build indexed document from pipeline result.
    fn build_indexed_document(
        &self,
        doc_id: String,
        result: crate::index::PipelineResult,
        format: DocumentFormat,
        name: Option<&str>,
        path: Option<&Path>,
    ) -> Result<IndexedDocument> {
        let tree = result
            .tree
            .ok_or_else(|| Error::Parse("Document tree not generated".to_string()))?;

        let node_count = tree.node_count();
        self.events.emit_index(IndexEvent::TreeBuilt { node_count });

        let doc_name = name
            .map(str::to_string)
            .or_else(|| {
                path.and_then(|p| p.file_stem())
                    .map(|s| s.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| result.name.clone());

        let mut doc = IndexedDocument::new(&doc_id, format)
            .with_name(&doc_name)
            .with_tree(tree)
            .with_metrics(result.metrics);

        doc.reasoning_index = result.reasoning_index;

        if let Some(p) = path {
            doc = doc.with_source_path(p);
        }

        if let Some(desc) = &result.description {
            doc = doc.with_description(desc);
        }

        if let Some(page_count) = result.page_count {
            doc = doc.with_page_count(page_count);
        }

        info!("Indexing complete: {} ({} nodes)", doc_id, node_count);
        self.events.emit_index(IndexEvent::Complete { doc_id });

        Ok(doc)
    }

    /// Detect document format from file extension.
    pub(crate) fn detect_format_from_path(&self, path: &Path) -> Result<DocumentFormat> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        DocumentFormat::from_extension(ext)
            .ok_or_else(|| Error::Parse(format!("Unsupported format: {}", ext)))
    }

    /// Convert [`IndexedDocument`] to [`PersistedDocument`].
    ///
    /// This is an associated function — it does not depend on client state.
    /// Stores content and logic fingerprints from the pipeline options.
    pub fn to_persisted(
        doc: IndexedDocument,
        pipeline_options: &PipelineOptions,
    ) -> PersistedDocument {
        let mut meta = DocumentMeta::new(&doc.id, &doc.name, doc.format.extension())
            .with_source_path(
                doc.source_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
            )
            .with_description(doc.description.clone().unwrap_or_default());

        // Compute content fingerprint for incremental indexing
        if let Some(ref path) = doc.source_path {
            if let Ok(bytes) = std::fs::read(path) {
                let fp = crate::utils::fingerprint::Fingerprint::from_bytes(&bytes);
                meta = meta.with_fingerprint(fp);
            }
        }

        // Store logic fingerprint (pipeline configuration hash)
        let logic_fp = pipeline_options.logic_fingerprint();
        meta = meta.with_logic_fingerprint(logic_fp);

        let tree = doc.tree.expect("IndexedDocument must have a tree");

        // Extract stats from metrics
        let node_count = tree.node_count();
        let (summary_tokens, duration_ms) = if let Some(ref m) = doc.metrics {
            (m.total_tokens_generated, m.total_time_ms())
        } else {
            (0, 0)
        };

        let mut persisted = PersistedDocument::new(meta, tree);

        for page in doc.pages {
            persisted.add_page(page.page, &page.content);
        }

        persisted.reasoning_index = doc.reasoning_index;
        persisted
            .meta
            .update_processing_stats(node_count, summary_tokens, duration_ms);

        persisted
    }
}

impl Clone for IndexerClient {
    fn clone(&self) -> Self {
        Self {
            executor_factory: Arc::clone(&self.executor_factory),
            events: self.events.clone(),
        }
    }
}
