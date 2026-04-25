// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document compile client.
//!
//! This module provides document compilation operations including
//! format detection, parsing, and tree building.
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::client::{IndexerClient, CompileInput};
//!
//! let indexer = IndexerClient::new(executor);
//!
//! let result = indexer
//!     .index(CompileInput::from_path("./document.md"))
//!     .await?;
//!
//! println!("Indexed: {} ({} nodes)", result.id, result.tree.as_ref().map(|t| t.node_count()).unwrap_or(0));
//! ```

use std::path::Path;
use std::sync::Arc;

use tracing::info;
use uuid::Uuid;

use vectorless_compiler::{CompilerInput, PipelineExecutor, PipelineOptions, SourceFormat};
use vectorless_document::{CURRENT_SCHEMA_VERSION, Document, DocumentFormat, DocumentMeta};
use vectorless_error::{Error, Result};
use vectorless_llm::LlmClient;
use vectorless_utils::fingerprint::Fingerprint;

use super::compile_input::CompileSource;
use vectorless_events::{CompileEvent, EventEmitter};

/// Document compile client.
///
/// Provides operations for compiling documents.
/// Each compile operation creates a fresh pipeline executor, enabling
/// true parallel document compilation without mutex contention.
pub(crate) struct IndexerClient {
    /// Factory for creating pipeline executors (one per compile operation).
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
        source: &CompileSource,
        name: Option<&str>,
        pipeline_options: PipelineOptions,
    ) -> Result<Document> {
        self.index_with_existing(source, name, pipeline_options, None)
            .await
    }

    /// Index a document, optionally reusing an existing tree for incremental updates.
    ///
    /// The caller provides fully constructed [`PipelineOptions`].
    pub async fn index_with_existing(
        &self,
        source: &CompileSource,
        name: Option<&str>,
        mut pipeline_options: PipelineOptions,
        existing_tree: Option<&vectorless_document::DocumentTree>,
    ) -> Result<Document> {
        pipeline_options.existing_tree = existing_tree.cloned();
        match source {
            CompileSource::Path(path) => self.index_from_path(path, name, pipeline_options).await,
            CompileSource::Content { data, format } => {
                self.index_from_content(data, *format, name, pipeline_options)
                    .await
            }
            CompileSource::Bytes { data, format } => {
                self.index_from_bytes(data, *format, name, pipeline_options)
                    .await
            }
        }
    }

    /// Index from a file path.
    ///
    /// Uses the format from `PipelineOptions.mode` — no redundant detection.
    async fn index_from_path(
        &self,
        path: &Path,
        name: Option<&str>,
        pipeline_options: PipelineOptions,
    ) -> Result<Document> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Validate file before compiling
        let validation = vectorless_utils::validate_file(&path)?;
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

        // Resolve format from pipeline options (set by Engine) — no re-detection
        let format = Self::format_from_mode(&pipeline_options.mode);

        let input = CompilerInput::file(&path);
        self.run_pipeline(
            input,
            format,
            &path.display().to_string(),
            name,
            Some(&path),
            pipeline_options,
        )
        .await
    }

    /// Index from content string.
    async fn index_from_content(
        &self,
        content: &str,
        format: DocumentFormat,
        name: Option<&str>,
        pipeline_options: PipelineOptions,
    ) -> Result<Document> {
        // Validate content before compiling
        let validation = vectorless_utils::validate_content(content, format);
        if !validation.valid {
            return Err(Error::Parse(
                validation
                    .errors
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Invalid content".to_string()),
            ));
        }

        let input = CompilerInput::content(content);
        self.run_pipeline(
            input,
            format,
            name.unwrap_or("content"),
            name,
            None,
            pipeline_options,
        )
        .await
    }

    /// Index from binary data.
    async fn index_from_bytes(
        &self,
        bytes: &[u8],
        format: DocumentFormat,
        name: Option<&str>,
        pipeline_options: PipelineOptions,
    ) -> Result<Document> {
        // Validate bytes before compiling
        let validation = vectorless_utils::validate_bytes(bytes, format);
        if !validation.valid {
            return Err(Error::Parse(
                validation
                    .errors
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Invalid bytes".to_string()),
            ));
        }

        info!(
            "Compiling {:?} document from bytes ({} bytes)",
            format,
            bytes.len()
        );

        let input = CompilerInput::bytes(bytes);
        self.run_pipeline(
            input,
            format,
            name.unwrap_or("bytes"),
            name,
            None,
            pipeline_options,
        )
        .await
    }

    /// Common pipeline execution: emit events → run pipeline → build result.
    #[tracing::instrument(skip_all, fields(format = ?format, source = %source_label))]
    async fn run_pipeline(
        &self,
        input: CompilerInput,
        format: DocumentFormat,
        source_label: &str,
        name: Option<&str>,
        path: Option<&Path>,
        pipeline_options: PipelineOptions,
    ) -> Result<Document> {
        self.events.emit_compile(CompileEvent::Started {
            path: source_label.to_string(),
        });

        let doc_id = Uuid::new_v4().to_string();
        self.events
            .emit_compile(CompileEvent::FormatDetected { format });

        info!("Compiling {:?} document: {}", format, source_label);

        let mut executor = (self.executor_factory)();
        let result = executor.execute(input, pipeline_options.clone()).await?;

        self.build_document(doc_id, result, format, name, path, &pipeline_options)
    }

    /// Build a Document from pipeline result.
    fn build_document(
        &self,
        doc_id: String,
        result: vectorless_compiler::CompileResult,
        format: DocumentFormat,
        name: Option<&str>,
        path: Option<&Path>,
        pipeline_options: &PipelineOptions,
    ) -> Result<Document> {
        let tree = result
            .tree
            .ok_or_else(|| Error::Parse("Document tree not generated".to_string()))?;

        let node_count = tree.node_count();
        self.events
            .emit_compile(CompileEvent::TreeBuilt { node_count });

        let doc_name = name
            .map(str::to_string)
            .or_else(|| {
                path.and_then(|p| p.file_stem())
                    .map(|s| s.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| result.name.clone());

        // Build DocumentMeta with fingerprints
        let mut meta = DocumentMeta::new();

        // Compute content fingerprint for incremental compilation (async I/O would be done
        // in the caller; here we just store the pipeline's logic fingerprint)
        let logic_fp = pipeline_options.logic_fingerprint();
        meta = meta.with_logic_fingerprint(logic_fp.to_string());

        // Extract stats from metrics
        let (summary_tokens, duration_ms) = (
            result.metrics.total_tokens_generated,
            result.metrics.total_time_ms(),
        );
        meta.update_processing_stats(node_count, summary_tokens, duration_ms);

        // Compute content fingerprint from source file if available
        if let Some(p) = path {
            if let Ok(bytes) = std::fs::read(p) {
                let fp = Fingerprint::from_bytes(&bytes);
                meta = meta.with_content_fingerprint(fp.to_string());
            }
        }

        let doc = Document {
            schema_version: CURRENT_SCHEMA_VERSION,
            doc_id,
            name: doc_name,
            format: format.extension().to_string(),
            source_path: path.map(|p| p.display().to_string()),
            tree,
            nav_index: result.navigation_index.unwrap_or_default(),
            reasoning_index: result.reasoning_index.unwrap_or_default(),
            summary: result.description.unwrap_or_default(),
            concepts: result.concepts,
            query_routes: result.query_routes,
            chain_index: result.chain_index,
            content_overlap: result.content_overlap,
            evidence_scores: result.evidence_scores,
            page_count: result.page_count,
            meta: Some(meta),
        };

        info!("Compiling complete: {} ({} nodes)", doc.doc_id, node_count);
        self.events.emit_compile(CompileEvent::Complete {
            doc_id: doc.doc_id.clone(),
        });

        Ok(doc)
    }

    /// Resolve `DocumentFormat` from `PipelineOptions.mode`.
    ///
    /// Falls back to Markdown for `Auto` mode (the engine resolves
    /// `Auto` to a concrete format before calling the indexer).
    fn format_from_mode(mode: &SourceFormat) -> DocumentFormat {
        match mode {
            SourceFormat::Markdown => DocumentFormat::Markdown,
            SourceFormat::Pdf => DocumentFormat::Pdf,
            SourceFormat::Auto => DocumentFormat::Markdown,
        }
    }

    /// Detect document format from file extension.
    pub(crate) fn detect_format_from_path(&self, path: &Path) -> Result<DocumentFormat> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        DocumentFormat::from_extension(ext)
            .ok_or_else(|| Error::Parse(format!("Unsupported format: {}", ext)))
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
