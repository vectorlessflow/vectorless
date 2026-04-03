// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Main Vectorless client for document indexing and retrieval.
//!
//! This module provides the high-level API for:
//! - Indexing documents (Markdown, PDF, DOCX, HTML)
//! - Retrieving document structure
//! - Querying documents with adaptive retrieval
//!
//! # Design
//!
//! The client uses **interior mutability** patterns to allow sharing across
//! async tasks while maintaining thread safety:
//!
//! - `Arc<RwLock<Workspace>>` - Thread-safe workspace access (multiple readers, single writer)
//! - `Arc<Mutex<PipelineExecutor>>` - Exclusive pipeline execution
//! - `Arc<AdaptiveRetriever>` - Immutable retriever (uses interior mutability internally)
//!
//! # Thread Safety
//!
//! `Vectorless` is `Clone + Send + Sync`. Cloning is cheap (reference count increment).
//! All clones share the same underlying resources.
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::client::{Vectorless, VectorlessBuilder};
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::core::Result<()> {
//! // Create a client
//! let client = VectorlessBuilder::new()
//!     .with_workspace("./my_workspace")
//!     .build()?;
//!
//! // Clone for use in multiple tasks (cheap - just Arc clone)
//! let client1 = client.clone();
//! let client2 = client.clone();
//!
//! // Can use concurrently
//! let doc_id = client.index("./document.md").await?;
//! let result = client.query(&doc_id, "What is this?").await?;
//! # Ok(())
//! # }
//! ```

use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use uuid::Uuid;
use tracing::info;

use crate::config::Config;
use crate::core::{DocumentTree, Result, Error};
use crate::parser::DocumentFormat;
use crate::storage::{Workspace, PersistedDocument, DocumentMeta as StorageMeta};
use crate::core::retriever::{AdaptiveRetriever, Retriever};
use crate::core::index::{PipelineExecutor, PipelineOptions, IndexInput, SummaryStrategy};

use super::types::{IndexMode, IndexOptions, DocumentInfo, QueryResult};

/// The main Vectorless client.
///
/// Provides high-level operations for document indexing and retrieval.
/// Uses interior mutability to allow sharing across async tasks.
///
/// # Cloning
///
/// Cloning is cheap - it only increments reference counts (`Arc`). All clones
/// share the same underlying resources (workspace, retriever, executor).
///
/// # Thread Safety
///
/// The client is `Clone + Send + Sync` and can be safely shared across
/// threads. All mutable state is protected by appropriate synchronization:
///
/// - Workspace: `Arc<RwLock<Workspace>>` - Multiple readers, single writer
/// - Executor: `Arc<Mutex<PipelineExecutor>>` - Exclusive access during indexing
/// - Retriever: `Arc<AdaptiveRetriever>` - Immutable, uses internal synchronization
pub struct Vectorless {
    /// Configuration (immutable, shared).
    config: Arc<Config>,

    /// Workspace for persistence (with built-in LRU cache).
    /// Uses RwLock for concurrent read access.
    workspace: Option<Arc<RwLock<Workspace>>>,

    /// Adaptive retriever (immutable, uses interior mutability internally).
    retriever: Arc<AdaptiveRetriever>,

    /// Pipeline executor for indexing.
    /// Uses Mutex for exclusive access during pipeline execution.
    executor: Arc<Mutex<PipelineExecutor>>,
}

impl Vectorless {
    /// Create a builder for custom configuration.
    #[must_use]
    pub fn builder() -> super::VectorlessBuilder {
        super::VectorlessBuilder::new()
    }

    /// Create a new client with default configuration.
    ///
    /// Note: Prefer using [`Vectorless::builder()`] for more control.
    fn new() -> Result<Self> {
        let config = Config::default();
        Ok(Self {
            config: Arc::new(config),
            workspace: None,
            retriever: Arc::new(AdaptiveRetriever::new()),
            executor: Arc::new(Mutex::new(PipelineExecutor::new())),
        })
    }

    // ============================================================
    // Document Indexing
    // ============================================================

    /// Index a document from a file path.
    ///
    /// Returns a unique document ID.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file does not exist
    /// - The file format is not supported
    /// - The pipeline execution fails
    pub async fn index(&self, path: impl AsRef<Path>) -> Result<String> {
        self.index_with_options(path, IndexOptions::default()).await
    }

    /// Index a document with custom options.
    ///
    /// # Errors
    ///
    /// See [`Vectorless::index`].
    pub async fn index_with_options(
        &self,
        path: impl AsRef<Path>,
        options: IndexOptions,
    ) -> Result<String> {
        let path = path.as_ref();
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        if !path.exists() {
            return Err(Error::Parse(format!("File not found: {}", path.display())));
        }

        // Generate document ID
        let doc_id = Uuid::new_v4().to_string();

        // Detect format
        let format = self.detect_format(&path, &options)?;

        info!("Indexing {:?} document: {}", format, path.display());

        // Convert client options to pipeline options
        let pipeline_options = PipelineOptions {
            mode: match options.mode {
                IndexMode::Auto => crate::core::index::IndexMode::Auto,
                IndexMode::Pdf => crate::core::index::IndexMode::Pdf,
                IndexMode::Markdown => crate::core::index::IndexMode::Markdown,
                IndexMode::Html => crate::core::index::IndexMode::Html,
                IndexMode::Docx => crate::core::index::IndexMode::Docx,
            },
            generate_ids: options.generate_ids,
            summary_strategy: if options.generate_summaries {
                SummaryStrategy::selective(
                    self.config.indexer.min_summary_tokens,
                    false,
                )
            } else {
                SummaryStrategy::none()
            },
            generate_description: options.generate_description,
            ..Default::default()
        };

        // Create pipeline input and execute (with mutex lock)
        let input = IndexInput::file(&path);
        let result = {
            let mut executor = self.executor.lock().map_err(|_| {
                Error::Other("Pipeline executor lock poisoned".to_string())
            })?;
            executor.execute(input, pipeline_options).await?
        };

        // Build persisted document
        let tree = result.tree.ok_or_else(|| {
            Error::Parse("Document tree not generated".to_string())
        })?;

        let meta = StorageMeta::new(&doc_id, &result.name, format.extension())
            .with_source_path(path.to_string_lossy().to_string())
            .with_description(result.description.clone().unwrap_or_default());

        let mut doc = PersistedDocument::new(meta, tree);

        // Add page count if available
        if let Some(page_count) = result.page_count {
            for i in 1..=page_count {
                doc.add_page(i, "");
            }
        }

        // Save to workspace if configured
        if let Some(ref workspace) = self.workspace {
            let mut ws = workspace.write().map_err(|_| {
                Error::Other("Workspace lock poisoned".to_string())
            })?;
            ws.add(&doc)?;
            info!("Saved document {} to workspace", doc_id);
        }

        info!("Indexing complete. Document ID: {}", doc_id);
        Ok(doc_id)
    }

    /// Detect document format from path and options.
    fn detect_format(&self, path: &Path, options: &IndexOptions) -> Result<DocumentFormat> {
        match options.mode {
            IndexMode::Auto => {
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                DocumentFormat::from_extension(ext)
                    .ok_or_else(|| Error::Parse(format!("Unknown format: {}", ext)))
            }
            IndexMode::Pdf => Ok(DocumentFormat::Pdf),
            IndexMode::Markdown => Ok(DocumentFormat::Markdown),
            IndexMode::Html => Ok(DocumentFormat::Html),
            IndexMode::Docx => Ok(DocumentFormat::Docx),
        }
    }

    // ============================================================
    // Document Retrieval
    // ============================================================

    /// Get a list of all indexed documents.
    #[must_use]
    pub fn list_documents(&self) -> Vec<DocumentInfo> {
        match &self.workspace {
            Some(workspace) => {
                let ws = match workspace.read() {
                    Ok(guard) => guard,
                    Err(_) => return Vec::new(),
                };
                ws.list_documents()
                    .iter()
                    .filter_map(|id| ws.get_meta(id))
                    .map(|meta| DocumentInfo {
                        id: meta.id.clone(),
                        name: meta.doc_name.clone(),
                        format: meta.doc_type.clone(),
                        description: meta.doc_description.clone(),
                        page_count: meta.page_count,
                        line_count: meta.line_count,
                    })
                    .collect()
            }
            None => Vec::new(),
        }
    }

    /// Get document structure (tree).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No workspace is configured
    /// - The document is not found
    pub fn get_structure(&self, doc_id: &str) -> Result<DocumentTree> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        let mut ws = workspace.write().map_err(|_| {
            Error::Other("Workspace lock poisoned".to_string())
        })?;

        let doc = ws.load(doc_id)?
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
    pub fn get_page_content(&self, doc_id: &str, pages: &str) -> Result<String> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        let mut ws = workspace.write().map_err(|_| {
            Error::Other("Workspace lock poisoned".to_string())
        })?;

        let doc = ws.load(doc_id)?
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
                    let start: usize = range[0].parse()
                        .map_err(|_| Error::Parse(format!("Invalid page number: {}", range[0])))?;
                    let end: usize = range[1].parse()
                        .map_err(|_| Error::Parse(format!("Invalid page number: {}", range[1])))?;
                    for p in start..=end {
                        result.push(p);
                    }
                }
            } else if !part.is_empty() {
                let page: usize = part.parse()
                    .map_err(|_| Error::Parse(format!("Invalid page number: {}", part)))?;
                result.push(page);
            }
        }

        Ok(result)
    }

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
        let tree = self.get_structure(doc_id)?;

        // Build retrieve options from config
        let retrieve_options = crate::core::retriever::RetrieveOptions::new()
            .with_top_k(self.config.retrieval.top_k)
            .with_content(true)
            .with_summaries(true);

        // Use adaptive retriever
        let response = self.retriever.retrieve(&tree, question, &retrieve_options).await
            .map_err(|e| Error::Retrieval(e.to_string()))?;

        // Extract node IDs and build content from results
        let node_ids: Vec<String> = response.results.iter()
            .filter_map(|r| r.node_id.clone())
            .collect();

        let content_parts: Vec<String> = response.results.iter()
            .map(|r| {
                let mut parts = vec![format!("## {}", r.title)];

                if let Some(ref summary) = r.summary {
                    parts.push(format!("Summary: {}", summary));
                }

                if let Some(ref content) = r.content {
                    parts.push(content.clone());
                }

                parts.join("\n\n")
            })
            .collect();

        let content = if content_parts.is_empty() {
            response.content
        } else {
            content_parts.join("\n\n---\n\n")
        };

        Ok(QueryResult {
            doc_id: doc_id.to_string(),
            node_ids,
            content,
            score: response.confidence,
        })
    }

    // ============================================================
    // Persistence
    // ============================================================

    /// Load a document from the workspace into cache.
    ///
    /// This preloads the document into the LRU cache for faster access.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub fn load(&self, doc_id: &str) -> Result<bool> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        let mut ws = workspace.write().map_err(|_| {
            Error::Other("Workspace lock poisoned".to_string())
        })?;

        if !ws.contains(doc_id) {
            return Ok(false);
        }

        let _ = ws.load(doc_id)?;
        Ok(true)
    }

    /// Remove a document from the workspace.
    ///
    /// # Errors
    ///
    /// Returns an error if no workspace is configured.
    pub fn remove(&self, doc_id: &str) -> Result<bool> {
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Config("No workspace configured".to_string()))?;

        let mut ws = workspace.write().map_err(|_| {
            Error::Other("Workspace lock poisoned".to_string())
        })?;
        ws.remove(doc_id)
    }

    /// Get the number of indexed documents.
    #[must_use]
    pub fn len(&self) -> usize {
        match &self.workspace {
            Some(workspace) => {
                let ws = match workspace.read() {
                    Ok(guard) => guard,
                    Err(_) => return 0,
                };
                ws.len()
            }
            None => 0,
        }
    }

    /// Check if there are no documents.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // ============================================================
    // Internal API (for Builder)
    // ============================================================

    /// Create a new client with the given components.
    pub(crate) fn with_components(
        config: Config,
        workspace: Option<Workspace>,
        retriever: AdaptiveRetriever,
        executor: PipelineExecutor,
    ) -> Self {
        Self {
            config: Arc::new(config),
            workspace: workspace.map(|w| Arc::new(RwLock::new(w))),
            retriever: Arc::new(retriever),
            executor: Arc::new(Mutex::new(executor)),
        }
    }
}

impl Clone for Vectorless {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            workspace: self.workspace.as_ref().map(Arc::clone),
            retriever: Arc::clone(&self.retriever),
            executor: Arc::clone(&self.executor),
        }
    }
}

impl Default for Vectorless {
    fn default() -> Self {
        Self::new().expect("Failed to create default Vectorless client")
    }
}

impl std::fmt::Debug for Vectorless {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vectorless")
            .field("has_workspace", &self.workspace.is_some())
            .field("doc_count", &self.len())
            .finish_non_exhaustive()
    }
}
