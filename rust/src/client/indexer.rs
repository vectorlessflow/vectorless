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

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tracing::info;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::index::{IndexInput, IndexMode, PipelineExecutor, PipelineOptions, SummaryStrategy};
use crate::parser::DocumentFormat;
use crate::storage::{DocumentMeta, PersistedDocument};

use super::context::ClientContext;
use super::events::{EventEmitter, IndexEvent};
use super::index_context::{IndexContext, IndexSource};
use super::types::{IndexOptions, IndexedDocument};

/// Document indexing client.
///
/// Provides operations for parsing and indexing documents.
pub struct IndexerClient {
    /// Pipeline executor.
    executor: Arc<Mutex<PipelineExecutor>>,

    /// Event emitter.
    events: EventEmitter,

    /// Configuration.
    config: IndexerConfig,
}

/// Indexer configuration.
#[derive(Debug, Clone)]
pub struct IndexerConfig {
    /// Minimum content tokens required to generate a summary.
    pub min_summary_tokens: usize,

    /// Whether to generate IDs by default.
    pub generate_ids: bool,

    /// Whether to generate descriptions by default.
    pub generate_descriptions: bool,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            min_summary_tokens: 20,
            generate_ids: true,
            generate_descriptions: false,
        }
    }
}

impl IndexerClient {
    /// Create a new indexer client.
    pub fn new(executor: PipelineExecutor) -> Self {
        Self {
            executor: Arc::new(Mutex::new(executor)),
            events: EventEmitter::new(),
            config: IndexerConfig::default(),
        }
    }

    /// Create with event emitter.
    pub fn with_events(mut self, events: EventEmitter) -> Self {
        self.events = events;
        self
    }

    /// Create with configuration.
    pub fn with_config(mut self, config: IndexerConfig) -> Self {
        self.config = config;
        self
    }

    /// Create from an existing executor Arc.
    pub(crate) fn from_arc(
        executor: Arc<Mutex<PipelineExecutor>>,
        events: EventEmitter,
        config: IndexerConfig,
    ) -> Self {
        Self {
            executor,
            events,
            config,
        }
    }

    /// Index a document from an index context.
    ///
    /// This is the main entry point for indexing documents. The context
    /// specifies the source (path, content, or bytes) and options.
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
    /// ```rust,ignore
    /// use vectorless::client::{IndexerClient, IndexContext};
    /// use vectorless::parser::DocumentFormat;
    ///
    /// // From file path
    /// let doc = indexer.index(IndexContext::from_path("./doc.md")).await?;
    ///
    /// // From HTML content
    /// let html = "<html><body><h1>Title</h1></body></html>";
    /// let doc = indexer.index(
    ///     IndexContext::from_content(html, DocumentFormat::Html)
    ///         .with_name("webpage")
    /// ).await?;
    /// ```
    pub async fn index(&self, ctx: IndexContext) -> Result<IndexedDocument> {
        match &ctx.source {
            IndexSource::Path(path) => self.index_from_path(path, &ctx).await,
            IndexSource::Content { data, format } => {
                self.index_from_content(data, *format, &ctx).await
            }
            IndexSource::Bytes { data, format } => self.index_from_bytes(data, *format, &ctx).await,
        }
    }

    /// Index from a file path.
    async fn index_from_path(&self, path: &Path, ctx: &IndexContext) -> Result<IndexedDocument> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        if !path.exists() {
            return Err(Error::Parse(format!("File not found: {}", path.display())));
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

        // Build pipeline options
        let pipeline_options = self.build_pipeline_options(&ctx.options, format);

        // Create pipeline input and execute
        let input = IndexInput::file(&path);
        let result = {
            let mut executor = self
                .executor
                .lock()
                .map_err(|_| Error::Other("Pipeline executor lock poisoned".to_string()))?;
            executor.execute(input, pipeline_options).await?
        };

        self.build_indexed_document(doc_id, result, format, ctx.name.as_deref(), Some(&path))
    }

    /// Index from content string.
    async fn index_from_content(
        &self,
        content: &str,
        format: DocumentFormat,
        ctx: &IndexContext,
    ) -> Result<IndexedDocument> {
        // Emit start event
        self.events.emit_index(IndexEvent::Started {
            path: ctx.name.clone().unwrap_or_else(|| "content".to_string()),
        });

        let doc_id = Uuid::new_v4().to_string();
        self.events
            .emit_index(IndexEvent::FormatDetected { format });

        info!("Indexing {:?} document from content", format);

        let pipeline_options = self.build_pipeline_options(&ctx.options, format);

        let input = IndexInput::content(content);
        let result = {
            let mut executor = self
                .executor
                .lock()
                .map_err(|_| Error::Other("Pipeline executor lock poisoned".to_string()))?;
            executor.execute(input, pipeline_options).await?
        };

        self.build_indexed_document(doc_id, result, format, ctx.name.as_deref(), None)
    }

    /// Index from binary data.
    async fn index_from_bytes(
        &self,
        bytes: &[u8],
        format: DocumentFormat,
        ctx: &IndexContext,
    ) -> Result<IndexedDocument> {
        // Emit start event
        self.events.emit_index(IndexEvent::Started {
            path: ctx.name.clone().unwrap_or_else(|| "bytes".to_string()),
        });

        let doc_id = Uuid::new_v4().to_string();
        self.events
            .emit_index(IndexEvent::FormatDetected { format });

        info!(
            "Indexing {:?} document from bytes ({} bytes)",
            format,
            bytes.len()
        );

        let pipeline_options = self.build_pipeline_options(&ctx.options, format);

        let input = IndexInput::bytes(bytes);
        let result = {
            let mut executor = self
                .executor
                .lock()
                .map_err(|_| Error::Other("Pipeline executor lock poisoned".to_string()))?;
            executor.execute(input, pipeline_options).await?
        };

        self.build_indexed_document(doc_id, result, format, ctx.name.as_deref(), None)
    }

    /// Build pipeline options from client options.
    fn build_pipeline_options(
        &self,
        options: &IndexOptions,
        format: DocumentFormat,
    ) -> PipelineOptions {
        PipelineOptions {
            mode: match format {
                DocumentFormat::Markdown => IndexMode::Markdown,
                DocumentFormat::Pdf => IndexMode::Pdf,
                DocumentFormat::Html => IndexMode::Html,
                DocumentFormat::Docx => IndexMode::Docx,
            },
            generate_ids: options.generate_ids,
            summary_strategy: if options.generate_summaries {
                SummaryStrategy::selective(self.config.min_summary_tokens, false)
            } else {
                SummaryStrategy::none()
            },
            generate_description: options.generate_description,
            ..Default::default()
        }
    }

    /// Build indexed document from pipeline result.
    fn build_indexed_document(
        &self,
        doc_id: String,
        result: crate::index::IndexResult,
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
            .with_tree(tree);

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
    fn detect_format_from_path(&self, path: &Path) -> Result<DocumentFormat> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        DocumentFormat::from_extension(ext)
            .ok_or_else(|| Error::Parse(format!("Unsupported format: {}", ext)))
    }

    /// Validate a document before indexing.
    ///
    /// # Errors
    ///
    /// Returns an error if the file doesn't exist or is not readable.
    pub fn validate(&self, path: impl AsRef<Path>) -> Result<ValidationResult> {
        let path = path.as_ref();

        if !path.exists() {
            return Ok(ValidationResult {
                valid: false,
                errors: vec![format!("File not found: {}", path.display())],
                warnings: vec![],
                format: None,
                estimated_size: 0,
            });
        }

        let metadata = std::fs::metadata(path)
            .map_err(|e| Error::Parse(format!("Cannot read file metadata: {}", e)))?;

        let estimated_size = metadata.len() as usize;
        let mut warnings = Vec::new();

        // Check file size
        if estimated_size > 100 * 1024 * 1024 {
            warnings.push("Large file (>100MB) may take longer to index".to_string());
        }

        // Detect format
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let format = DocumentFormat::from_extension(ext);

        if format.is_none() {
            return Ok(ValidationResult {
                valid: false,
                errors: vec![format!("Unsupported format: {}", ext)],
                warnings,
                format: None,
                estimated_size,
            });
        }

        Ok(ValidationResult {
            valid: true,
            errors: vec![],
            warnings,
            format,
            estimated_size,
        })
    }

    /// Convert IndexedDocument to PersistedDocument for storage.
    pub fn to_persisted(&self, doc: IndexedDocument) -> PersistedDocument {
        let meta = DocumentMeta::new(&doc.id, &doc.name, doc.format.extension())
            .with_source_path(
                doc.source_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
            )
            .with_description(doc.description.clone().unwrap_or_default());

        let mut persisted =
            PersistedDocument::new(meta, doc.tree.expect("IndexedDocument must have a tree"));

        for page in doc.pages {
            persisted.add_page(page.page, &page.content);
        }

        persisted
    }

    /// Get the underlying executor Arc (for advanced use).
    pub(crate) fn inner(&self) -> Arc<Mutex<PipelineExecutor>> {
        Arc::clone(&self.executor)
    }
}

impl Clone for IndexerClient {
    fn clone(&self) -> Self {
        Self {
            executor: Arc::clone(&self.executor),
            events: self.events.clone(),
            config: self.config.clone(),
        }
    }
}

/// Document validation result.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the document is valid for indexing.
    pub valid: bool,

    /// Validation errors (prevents indexing).
    pub errors: Vec<String>,

    /// Validation warnings (non-blocking).
    pub warnings: Vec<String>,

    /// Detected document format.
    pub format: Option<DocumentFormat>,

    /// Estimated file size in bytes.
    pub estimated_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexer_client_creation() {
        let executor = PipelineExecutor::new();
        let client = IndexerClient::new(executor);
        assert_eq!(client.config.min_summary_tokens, 20);
    }

    #[test]
    fn test_validate_missing_file() {
        let executor = PipelineExecutor::new();
        let client = IndexerClient::new(executor);

        let result = client.validate("./nonexistent.md").unwrap();
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }
}
