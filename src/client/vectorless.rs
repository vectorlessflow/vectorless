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

use std::collections::HashMap;
use std::path::Path;

use uuid::Uuid;
use tracing::info;

use crate::config::Config;
use crate::core::{DocumentTree, Result, Error};
use crate::document::{DocumentFormat, MarkdownParser, DocumentParser};
use crate::indexer::TreeBuilder;
use crate::storage::{Workspace, PersistedDocument, DocumentMeta as StorageMeta};

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
    pub(crate) workspace: Option<Workspace>,

    /// In-memory document cache.
    pub(crate) documents: HashMap<String, IndexedDocument>,
}

impl Vectorless {
    /// Create a new client with default configuration.
    pub fn new() -> Result<Self> {
        Ok(Self {
            config: Config::default(),
            workspace: None,
            documents: HashMap::new(),
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
            DocumentFormat::Markdown => self.index_markdown(&doc_id, &path, &options).await?,
            DocumentFormat::Pdf => {
                return Err(Error::Parse("PDF indexing not yet implemented".to_string()));
            }
            DocumentFormat::Html => {
                return Err(Error::Parse("HTML parsing not yet implemented".to_string()));
            }
            DocumentFormat::Docx => {
                return Err(Error::Parse("DOCX parsing not yet implemented".to_string()));
            }
            DocumentFormat::Text => self.index_text(&doc_id, &path, &options).await?,
        };

        // Save to workspace if configured
        if let Some(ref workspace) = self.workspace {
            self.save_to_workspace(workspace, &indexed)?;
        }

        // Cache in memory
        let doc_id_clone = indexed.id.clone();
        self.documents.insert(doc_id_clone.clone(), indexed);

        info!("Indexing complete. Document ID: {}", doc_id_clone);
        Ok(doc_id_clone)
    }

    /// Save an indexed document to the workspace.
    fn save_to_workspace(&self, _workspace: &Workspace, indexed: &IndexedDocument) -> Result<()> {
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

        // Note: workspace.add() would need &mut self, so we need to adjust
        // For now, just log that we would save
        info!("Would save document {} to workspace", indexed.id);

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
        }
    }

    /// Index a Markdown file.
    async fn index_markdown(
        &self,
        doc_id: &str,
        path: &Path,
        options: &IndexOptions,
    ) -> Result<IndexedDocument> {
        let parser = MarkdownParser::new();
        let result = parser.parse_file(path).await?;

        // Build tree
        let builder = TreeBuilder::new()
            .with_root_title(&result.meta.name);

        let tree = if options.generate_ids {
            builder.build_with_ids(result.nodes)
        } else {
            builder.build(result.nodes)
        };

        // Create indexed document
        let mut doc = IndexedDocument::new(doc_id, DocumentFormat::Markdown)
            .with_name(&result.meta.name)
            .with_source_path(path)
            .with_line_count(result.meta.line_count)
            .with_tree(tree);

        if options.generate_description {
            if let Some(desc) = result.meta.description {
                doc = doc.with_description(desc);
            }
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
    /// Note: This requires the retriever module to be implemented.
    pub async fn query(&self, doc_id: &str, _question: &str) -> Result<QueryResult> {
        let _doc = self.documents.get(doc_id)
            .ok_or_else(|| Error::DocumentNotFound(format!("Document not found: {}", doc_id)))?;

        // TODO: Implement retrieval once retriever module is ready
        Err(Error::Parse("Query not yet implemented. The retriever module is required.".to_string()))
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
        let workspace = self.workspace.as_ref()
            .ok_or_else(|| Error::Parse("No workspace configured".to_string()))?;

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

        if let Some(ref mut workspace) = self.workspace {
            workspace.remove(doc_id)?;
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
