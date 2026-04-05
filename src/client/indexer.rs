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
//! let indexer = IndexerClient::new(executor);
//!
//! let result = indexer
//!     .index("./document.md")
//!     .with_summaries()
//!     .await?;
//!
//! println!("Indexed: {} ({} nodes)", result.doc_id, result.node_count);
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
use super::types::{IndexOptions, IndexMode as ClientIndexMode, IndexedDocument};

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

    /// Index a document from a file path.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file does not exist
    /// - The file format is not supported
    /// - The pipeline execution fails
    pub async fn index(&self, path: impl AsRef<Path>) -> Result<IndexedDocument> {
        self.index_with_options(path, IndexOptions::default()).await
    }

    /// Index a document with custom options.
    ///
    /// # Errors
    ///
    /// See [`IndexerClient::index`].
    pub async fn index_with_options(
        &self,
        path: impl AsRef<Path>,
        options: IndexOptions,
    ) -> Result<IndexedDocument> {
        let path = path.as_ref();
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

        // Detect format
        let format = self.detect_format(&path, &options)?;
        self.events.emit_index(IndexEvent::FormatDetected { format });

        info!("Indexing {:?} document: {}", format, path.display());

        // Convert client options to pipeline options
        let pipeline_options = PipelineOptions {
            mode: match options.mode {
                ClientIndexMode::Auto => IndexMode::Auto,
                ClientIndexMode::Pdf => IndexMode::Pdf,
                ClientIndexMode::Markdown => IndexMode::Markdown,
                ClientIndexMode::Html => IndexMode::Html,
                ClientIndexMode::Docx => IndexMode::Docx,
            },
            generate_ids: options.generate_ids,
            summary_strategy: if options.generate_summaries {
                SummaryStrategy::selective(self.config.min_summary_tokens, false)
            } else {
                SummaryStrategy::none()
            },
            generate_description: options.generate_description,
            ..Default::default()
        };

        // Create pipeline input and execute
        let input = IndexInput::file(&path);
        let result = {
            let mut executor = self.executor.lock()
                .map_err(|_| Error::Other("Pipeline executor lock poisoned".to_string()))?;
            executor.execute(input, pipeline_options).await?
        };

        // Build indexed document
        let tree = result
            .tree
            .ok_or_else(|| Error::Parse("Document tree not generated".to_string()))?;

        let node_count = tree.node_count();
        self.events.emit_index(IndexEvent::TreeBuilt { node_count });

        let mut doc = IndexedDocument::new(&doc_id, format)
            .with_name(&result.name)
            .with_source_path(&path)
            .with_tree(tree);

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

    /// Detect document format from path and options.
    pub fn detect_format(&self, path: &Path, options: &IndexOptions) -> Result<DocumentFormat> {
        match options.mode {
            ClientIndexMode::Auto => {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                DocumentFormat::from_extension(ext)
                    .ok_or_else(|| Error::Parse(format!("Unknown format: {}", ext)))
            }
            ClientIndexMode::Pdf => Ok(DocumentFormat::Pdf),
            ClientIndexMode::Markdown => Ok(DocumentFormat::Markdown),
            ClientIndexMode::Html => Ok(DocumentFormat::Html),
            ClientIndexMode::Docx => Ok(DocumentFormat::Docx),
        }
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
                errors: vec![format!("Unknown format: {}", ext)],
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

        let mut persisted = PersistedDocument::new(
            meta,
            doc.tree.expect("IndexedDocument must have a tree"),
        );

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
