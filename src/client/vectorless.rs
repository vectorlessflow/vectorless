// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Main Vectorless client for document indexing and retrieval.
//!
//! This module provides the high-level API for:
//! - Indexing documents (Markdown, PDF)
//! - Retrieving document structure
//! - Querying documents
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::client::{Vectorless, VectorlessBuilder};
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::core::Result<()> {
//! // Create a client
//! let mut client = VectorlessBuilder::new()
//!     .with_workspace("./my_workspace")
//!     .build()?;
//!
//! // Index a document
//! let doc_id = client.index("./document.md").await?;
//!
//! // Get document structure
//! let structure = client.get_structure(&doc_id)?;
//!
//! // List documents
//! for doc in client.list_documents() {
//!     println!("{}: {}", doc.id, doc.name);
//! }
//! # Ok(())
//! # }
//! ```

use std::path::Path;

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
/// Uses Workspace for persistence with built-in LRU cache.
pub struct Vectorless {
    /// Configuration.
    pub(crate) config: Config,

    /// Workspace for persistence (with built-in LRU cache).
    pub(crate) workspace: Option<Workspace>,

    /// Adaptive retriever.
    pub(crate) retriever: AdaptiveRetriever,

    /// Pipeline executor for indexing.
    pub(crate) executor: PipelineExecutor,
}

impl Vectorless {
    /// Create a new client with default configuration.
    fn new() -> Result<Self> {
        let config = Config::default();
        Ok(Self {
            config,
            workspace: None,
            retriever: AdaptiveRetriever::new(),
            executor: PipelineExecutor::new(),
        })
    }

    /// Create a builder for custom configuration.
    pub fn builder() -> super::VectorlessBuilder {
        super::VectorlessBuilder::new()
    }

    // ============================================================
    // Document Indexing
    // ============================================================

    /// Index a document from a file path.
    ///
    /// Returns a unique document ID.
    pub async fn index(&mut self, path: impl AsRef<Path>) -> Result<String> {
        self.index_with_options(path, IndexOptions::default()).await
    }

    /// Index a document with custom options.
    pub async fn index_with_options(
        &mut self,
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
                SummaryStrategy::selective(100, true)
            } else {
                SummaryStrategy::none()
            },
            generate_description: options.generate_description,
            ..Default::default()
        };

        // Create pipeline input and execute
        let input = IndexInput::file(&path);
        let result = self.executor.execute(input, pipeline_options).await?;

        // Build persisted document
        let tree = result.tree.ok_or_else(||
            Error::Parse("Document tree not generated".to_string()))?;

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
        if let Some(ref mut workspace) = self.workspace {
            workspace.add(&doc)?;
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
    pub fn list_documents(&self) -> Vec<DocumentInfo> {
        match &self.workspace {
            Some(workspace) => workspace.list_documents()
                .iter()
                .filter_map(|id| workspace.get_meta(id))
                .map(|meta| DocumentInfo {
                    id: meta.id.clone(),
                    name: meta.doc_name.clone(),
                    format: meta.doc_type.clone(),
                    description: meta.doc_description.clone(),
                    page_count: meta.page_count,
                    line_count: meta.line_count,
                })
                .collect(),
            None => Vec::new(),
        }
    }

    /// Get document structure (tree).
    pub fn get_structure(&mut self, doc_id: &str) -> Result<DocumentTree> {
        let workspace = self.workspace.as_mut()
            .ok_or_else(|| Error::Parse("No workspace configured".to_string()))?;

        let doc = workspace.load(doc_id)?
            .ok_or_else(|| Error::DocumentNotFound(format!("Document not found: {}", doc_id)))?;

        Ok(doc.tree)
    }

    /// Get page content for PDFs.
    pub fn get_page_content(&mut self, doc_id: &str, pages: &str) -> Result<String> {
        let workspace = self.workspace.as_mut()
            .ok_or_else(|| Error::Parse("No workspace configured".to_string()))?;

        let doc = workspace.load(doc_id)?
            .ok_or_else(|| Error::DocumentNotFound(format!("Document not found: {}", doc_id)))?;

        if doc.pages.is_empty() {
            return Err(Error::Parse("No page content available".to_string()));
        }

        // Parse page range
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
    /// Uses the retriever type configured in `RetrievalConfig`.
    pub async fn query(&mut self, doc_id: &str, question: &str) -> Result<QueryResult> {
        let tree = self.get_structure(doc_id)?;

        // Build retrieve options from config
        let retrieve_options = crate::core::retriever::RetrieveOptions::new()
            .with_top_k(self.config.retrieval.top_k)
            .with_content(true)
            .with_summaries(true);

        // Use adaptive retriever directly
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
            response.content // Use pre-aggregated content if available
        } else {
            content_parts.join("\n\n---\n\n")
        };

        // Use confidence score from response
        let avg_score = response.confidence;

        Ok(QueryResult {
            doc_id: doc_id.to_string(),
            node_ids,
            content,
            score: avg_score,
        })
    }

    // ============================================================
    // Persistence
    // ============================================================

    /// Save all documents to the workspace.
    pub fn save(&self) -> Result<()> {
        if self.workspace.is_none() {
            return Err(Error::Parse("No workspace configured".to_string()));
        }

        // Documents are saved automatically when indexed
        Ok(())
    }

    /// Load a document from the workspace into cache.
    ///
    /// This preloads the document into the LRU cache for faster access.
    pub fn load(&mut self, doc_id: &str) -> Result<bool> {
        let workspace = self.workspace.as_mut()
            .ok_or_else(|| Error::Parse("No workspace configured".to_string()))?;

        if !workspace.contains(doc_id) {
            return Ok(false);
        }

        // Load into cache
        let _ = workspace.load(doc_id)?;
        Ok(true)
    }

    /// Remove a document.
    pub fn remove(&mut self, doc_id: &str) -> Result<bool> {
        match &mut self.workspace {
            Some(workspace) => workspace.remove(doc_id),
            None => Ok(false),
        }
    }

    /// Get the number of indexed documents.
    pub fn len(&self) -> usize {
        match &self.workspace {
            Some(workspace) => workspace.len(),
            None => 0,
        }
    }

    /// Check if there are no documents.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for Vectorless {
    fn default() -> Self {
        Self::new().expect("Failed to create default Vectorless client")
    }
}
