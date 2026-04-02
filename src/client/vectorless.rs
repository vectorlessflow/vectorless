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
//!     .with_api_key("sk-...")
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

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use uuid::Uuid;
use tracing::info;

use crate::config::Config;
use crate::core::{DocumentTree, Result, Error};
use crate::document::DocumentFormat;
use crate::storage::{Workspace, PersistedDocument, DocumentMeta as StorageMeta};
use crate::document::ParserRegistry;
use crate::core::retriever::{AdaptiveRetriever, Retriever};
use crate::core::index::{PipelineExecutor, PipelineOptions, IndexInput, SummaryStrategy};

use super::types::{IndexedDocument, IndexMode, IndexOptions, DocumentInfo, QueryResult};

/// The main Vectorless client.
///
/// Provides high-level operations for document indexing and retrieval.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Vectorless {
    /// Configuration.
    pub(crate) config: Config,

    /// Optional workspace for persistence.
    pub(crate) workspace: Option<RefCell<Workspace>>,

    /// In-memory document cache.
    pub(crate) documents: HashMap<String, IndexedDocument>,

    /// Parser registry.
    pub(crate) parser_registry: ParserRegistry,

    /// Adaptive retriever.
    pub(crate) retriever: AdaptiveRetriever,
}

impl Vectorless {
    /// Create a new client with default configuration.
    pub fn new() -> Result<Self> {
        let config = Config::default();
        Ok(Self {
            config,
            workspace: None,
            documents: HashMap::new(),
            parser_registry: ParserRegistry::with_defaults(),
            retriever: AdaptiveRetriever::new(),
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

        // Parse based on format
        let indexed = match format {
            DocumentFormat::Markdown => self.index_document(&doc_id, &path, &options, DocumentFormat::Markdown).await?,
            DocumentFormat::Pdf => self.index_document(&doc_id, &path, &options, DocumentFormat::Pdf).await?,
            DocumentFormat::Html => {
                return Err(Error::Parse("HTML parsing not yet implemented".to_string()));
            }
            DocumentFormat::Docx => self.index_document(&doc_id, &path, &options, DocumentFormat::Docx).await?,
            DocumentFormat::Text => self.index_text(&doc_id, &path, &options).await?,
        };

        // Save to workspace if configured
        if let Some(ref workspace_cell) = self.workspace {
            let mut workspace = workspace_cell.borrow_mut();
            self.save_to_workspace(&mut workspace, &indexed)?;
        }

        // Cache in memory
        let doc_id_clone = indexed.id.clone();
        self.documents.insert(doc_id_clone.clone(), indexed);

        info!("Indexing complete. Document ID: {}", doc_id_clone);
        Ok(doc_id_clone)
    }

    /// Save an indexed document to the workspace.
    fn save_to_workspace(&self, workspace: &mut Workspace, indexed: &IndexedDocument) -> Result<()> {
        let tree = indexed.tree.as_ref()
            .ok_or_else(|| Error::Parse("Document tree not generated".to_string()))?;

        let meta = StorageMeta::new(
            &indexed.id,
            &indexed.name,
            indexed.format.extension()
        )
        .with_source_path(indexed.source_path.clone().unwrap_or_default())
        .with_description(indexed.description.clone().unwrap_or_default());

        let mut doc = PersistedDocument::new(meta, tree.clone());

        // Add pages if available
        for page in &indexed.pages {
            doc.add_page(page.page, &page.content);
        }

        workspace.add(&doc)?;
        info!("Saved document {} to workspace", indexed.id);

        Ok(())
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

    /// Index a document file (Markdown, PDF, etc).
    async fn index_document(
        &self,
        doc_id: &str,
        path: &Path,
        options: &IndexOptions,
        format: DocumentFormat,
    ) -> Result<IndexedDocument> {
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

        // Create pipeline input
        let input = IndexInput::file(path);

        // Execute pipeline
        let mut executor = PipelineExecutor::new();
        let result = executor.execute(input, pipeline_options).await?;

        // Create indexed document from result
        let mut doc = IndexedDocument::new(doc_id, format)
            .with_name(&result.name)
            .with_source_path(path);

        if let Some(tree) = result.tree {
            doc = doc.with_tree(tree);
        }

        if let Some(page_count) = result.page_count {
            doc = doc.with_page_count(page_count);
        }

        if let Some(line_count) = result.line_count {
            doc = doc.with_line_count(line_count);
        }

        if let Some(desc) = result.description {
            doc = doc.with_description(desc);
        }

        Ok(doc)
    }

    /// Index a plain text file.
    async fn index_text(
        &self,
        doc_id: &str,
        path: &Path,
        _options: &IndexOptions,
    ) -> Result<IndexedDocument> {
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| Error::Io(e))?;

        let line_count = content.lines().count();

        // Create a simple tree with root containing all content
        let tree = DocumentTree::new(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Document"),
            &content,
        );

        let doc = IndexedDocument::new(doc_id, DocumentFormat::Text)
            .with_name(path.file_name().unwrap().to_string_lossy())
            .with_source_path(path)
            .with_line_count(line_count)
            .with_tree(tree);

        Ok(doc)
    }

    // ============================================================
    // Document Retrieval
    // ============================================================

    /// Get a list of all indexed documents.
    pub fn list_documents(&self) -> Vec<DocumentInfo> {
        self.documents.values().map(|doc| DocumentInfo {
            id: doc.id.clone(),
            name: doc.name.clone(),
            format: doc.format.extension().to_string(),
            description: doc.description.clone(),
            page_count: doc.page_count,
            line_count: doc.line_count,
        }).collect()
    }

    /// Get document metadata.
    pub fn get_document(&self, doc_id: &str) -> Option<&IndexedDocument> {
        self.documents.get(doc_id)
    }

    /// Get document structure (tree).
    pub fn get_structure(&self, doc_id: &str) -> Result<&DocumentTree> {
        let doc = self.documents.get(doc_id)
            .ok_or_else(|| Error::DocumentNotFound(format!("Document not found: {}", doc_id)))?;

        doc.tree.as_ref()
            .ok_or_else(|| Error::Parse("Document tree not loaded".to_string()))
    }

    /// Get page content for PDFs.
    pub fn get_page_content(&self, doc_id: &str, pages: &str) -> Result<String> {
        let doc = self.documents.get(doc_id)
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
    pub async fn query(&self, doc_id: &str, question: &str) -> Result<QueryResult> {
        let doc = self.documents.get(doc_id)
            .ok_or_else(|| Error::DocumentNotFound(format!("Document not found: {}", doc_id)))?;

        let tree = doc.tree.as_ref()
            .ok_or_else(|| Error::Parse("Document tree not loaded".to_string()))?;

        // Build retrieve options from config
        let retrieve_options = crate::core::retriever::RetrieveOptions::new()
            .with_top_k(self.config.retrieval.top_k)
            .with_content(true)
            .with_summaries(true);

        // Use adaptive retriever directly
        let response = Retriever::retrieve(&self.retriever, tree, question, &retrieve_options).await
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

    /// Load a document from the workspace.
    pub fn load(&mut self, doc_id: &str) -> Result<bool> {
        let workspace_cell = self.workspace.as_ref()
            .ok_or_else(|| Error::Parse("No workspace configured".to_string()))?;

        let mut workspace = workspace_cell.borrow_mut();

        if !workspace.contains(doc_id) {
            return Ok(false);
        }

        let persisted = workspace.load(doc_id)?
            .ok_or_else(|| Error::DocumentNotFound(format!("Document not found: {}", doc_id)))?;

        let format = DocumentFormat::from_extension(&persisted.meta.format)
            .unwrap_or(DocumentFormat::Text);

        let doc = IndexedDocument::new(&persisted.meta.id, format)
            .with_name(&persisted.meta.name)
            .with_tree(persisted.tree);

        self.documents.insert(doc_id.to_string(), doc);
        Ok(true)
    }

    /// Remove a document.
    pub fn remove(&mut self, doc_id: &str) -> Result<bool> {
        let existed = self.documents.remove(doc_id).is_some();

        if let Some(ref workspace_cell) = self.workspace {
            workspace_cell.borrow_mut().remove(doc_id)?;
        }

        Ok(existed)
    }

    /// Get the number of indexed documents.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Check if there are no documents.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }
}

impl Default for Vectorless {
    fn default() -> Self {
        Self::new().expect("Failed to create default Vectorless client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = Vectorless::new().unwrap();
        assert!(client.is_empty());
    }

    #[test]
    fn test_parse_page_range() {
        let client = Vectorless::new().unwrap();

        let pages = client.parse_page_range("5").unwrap();
        assert_eq!(pages, vec![5]);

        let pages = client.parse_page_range("5-7").unwrap();
        assert_eq!(pages, vec![5, 6, 7]);

        let pages = client.parse_page_range("3,8,12").unwrap();
        assert_eq!(pages, vec![3, 8, 12]);

        let pages = client.parse_page_range("5-7,10").unwrap();
        assert_eq!(pages, vec![5, 6, 7, 10]);
    }
}
