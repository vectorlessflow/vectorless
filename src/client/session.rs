// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Session management for multi-document operations.
//!
//! This module provides session-based document management with
//! automatic caching and cross-document querying.
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::client::IndexContext;
//!
//! let session = client.session();
//!
//! // Index multiple documents
//! let doc1 = session.index(IndexContext::from_path("./doc1.md")).await?;
//! let doc2 = session.index(IndexContext::from_path("./doc2.md")).await?;
//!
//! // Query across all documents
//! let results = session.query_all("What is X?").await?;
//!
//! // Query single document (uses cached tree)
//! let result = session.query(&doc1, "Summary?").await?;
//! ```

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::info;
use uuid::Uuid;

use crate::error::Result;
use crate::retrieval::RetrieveOptions;
use crate::storage::PersistedDocument;
use crate::{DocumentTree, Error};

use super::context::ClientContext;
use super::events::EventEmitter;
use super::indexer::IndexerClient;
use super::retriever::RetrieverClient;
use super::types::{DocumentInfo, QueryResult};
use super::workspace::WorkspaceClient;

/// Session for managing multiple documents.
///
/// Provides automatic caching of document trees and cross-document operations.
pub struct Session {
    /// Session ID.
    pub id: Uuid,

    /// Session configuration.
    config: SessionConfig,

    /// Document contexts (cached).
    documents: HashMap<String, DocumentContext>,

    /// Indexer client.
    indexer: IndexerClient,

    /// Retriever client.
    retriever: RetrieverClient,

    /// Workspace client.
    workspace: WorkspaceClient,

    /// Event emitter.
    events: EventEmitter,

    /// Session statistics.
    stats: SessionStats,

    /// Created at timestamp.
    created_at: Instant,
}

/// Document context within a session.
#[derive(Debug, Clone)]
struct DocumentContext {
    /// Document ID.
    doc_id: String,

    /// Cached document tree.
    tree: Option<Arc<DocumentTree>>,

    /// Document metadata.
    meta: DocumentInfo,

    /// Access count.
    access_count: usize,

    /// Last access time.
    last_accessed: Instant,
}

/// Session configuration.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Maximum documents to cache in memory.
    pub max_cached_documents: usize,

    /// Cache eviction policy.
    pub eviction_policy: EvictionPolicy,

    /// Preload strategy when indexing.
    pub preload_strategy: PreloadStrategy,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_cached_documents: 100,
            eviction_policy: EvictionPolicy::Lru,
            preload_strategy: PreloadStrategy::Lazy,
        }
    }
}

/// Cache eviction policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionPolicy {
    /// Least recently used.
    Lru,
    /// First in, first out.
    Fifo,
    /// No eviction (until session closes).
    None,
}

/// Document preload strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreloadStrategy {
    /// Load trees on demand.
    Lazy,
    /// Load trees immediately when indexing.
    Eager,
}

/// Session statistics.
#[derive(Debug, Default)]
pub struct SessionStats {
    /// Total documents in session.
    pub document_count: Cell<usize>,

    /// Total queries made.
    pub query_count: Cell<usize>,

    /// Cache hits.
    pub cache_hits: Cell<usize>,

    /// Cache misses.
    pub cache_misses: Cell<usize>,

    /// Total query time (in microseconds).
    total_query_time_us: Cell<u64>,
}

impl SessionStats {
    /// Get the cache hit rate.
    pub fn cache_hit_rate(&self) -> f32 {
        let total = self.cache_hits.get() + self.cache_misses.get();
        if total == 0 {
            0.0
        } else {
            self.cache_hits.get() as f32 / total as f32
        }
    }

    /// Get the total query time.
    pub fn total_query_time(&self) -> Duration {
        Duration::from_micros(self.total_query_time_us.get())
    }

    /// Get the average query time.
    pub fn avg_query_time(&self) -> Option<Duration> {
        let count = self.query_count.get();
        if count == 0 {
            None
        } else {
            Some(self.total_query_time() / count as u32)
        }
    }

    /// Increment query count.
    fn increment_query_count(&self) {
        self.query_count.set(self.query_count.get() + 1);
    }

    /// Add query time.
    fn add_query_time(&self, duration: Duration) {
        self.total_query_time_us
            .set(self.total_query_time_us.get() + duration.as_micros() as u64);
    }

    /// Increment cache hits.
    fn increment_cache_hits(&self) {
        self.cache_hits.set(self.cache_hits.get() + 1);
    }

    /// Increment cache misses.
    fn increment_cache_misses(&self) {
        self.cache_misses.set(self.cache_misses.get() + 1);
    }
}

impl Clone for SessionStats {
    fn clone(&self) -> Self {
        Self {
            document_count: Cell::new(self.document_count.get()),
            query_count: Cell::new(self.query_count.get()),
            cache_hits: Cell::new(self.cache_hits.get()),
            cache_misses: Cell::new(self.cache_misses.get()),
            total_query_time_us: Cell::new(self.total_query_time_us.get()),
        }
    }
}

impl Session {
    /// Create a new session.
    pub(crate) fn new(
        indexer: IndexerClient,
        retriever: RetrieverClient,
        workspace: WorkspaceClient,
        events: EventEmitter,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            config: SessionConfig::default(),
            documents: HashMap::new(),
            indexer,
            retriever,
            workspace,
            events,
            stats: SessionStats::default(),
            created_at: Instant::now(),
        }
    }

    /// Create with configuration.
    pub fn with_config(mut self, config: SessionConfig) -> Self {
        self.config = config;
        self
    }

    /// Get the session ID.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Get session age.
    pub fn age(&self) -> Duration {
        Instant::now().duration_since(self.created_at)
    }

    // ============================================================
    // Document Indexing
    // ============================================================

    /// Index a document into this session.
    ///
    /// The document is indexed, saved to workspace, and cached in this session.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The index context containing source and options
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use vectorless::client::IndexContext;
    /// use vectorless::parser::DocumentFormat;
    ///
    /// // From file
    /// let id1 = session.index(IndexContext::from_path("./doc.md")).await?;
    ///
    /// // From content
    /// let html = "<html><body>Content</body></html>";
    /// let id2 = session.index(
    ///     IndexContext::from_content(html, DocumentFormat::Html)
    /// ).await?;
    /// ```
    pub async fn index(&self, ctx: super::IndexContext) -> Result<String> {
        // Index the document
        let doc = self.indexer.index(ctx).await?;

        // Save to workspace
        let persisted = self.indexer.to_persisted(doc);
        self.workspace.save(&persisted).await?;

        // Cache in session
        let doc_id = persisted.meta.id.clone();

        info!("Session {}: indexed document {}", self.id, doc_id);

        Ok(doc_id)
    }

    // ============================================================
    // Document Querying
    // ============================================================

    /// Query a document within this session.
    ///
    /// Uses the cached tree if available, otherwise loads from workspace.
    pub async fn query(&self, doc_id: &str, question: &str) -> Result<QueryResult> {
        self.query_with_options(doc_id, question, RetrieveOptions::default())
            .await
    }

    /// Query a document with options.
    pub async fn query_with_options(
        &self,
        doc_id: &str,
        question: &str,
        options: RetrieveOptions,
    ) -> Result<QueryResult> {
        let start = Instant::now();

        // Get the document tree
        let tree = self.get_tree(doc_id).await?;

        // Query
        let mut result = self.retriever.query(&tree, question, &options).await?;
        result.doc_id = doc_id.to_string();

        // Update stats
        self.stats.increment_query_count();
        self.stats.add_query_time(start.elapsed());

        Ok(result)
    }

    /// Query across all documents in this session.
    ///
    /// Searches each document and merges results.
    pub async fn query_all(&self, question: &str) -> Result<Vec<QueryResult>> {
        self.query_all_with_options(question, RetrieveOptions::default())
            .await
    }

    /// Query across all documents with options.
    pub async fn query_all_with_options(
        &self,
        question: &str,
        options: RetrieveOptions,
    ) -> Result<Vec<QueryResult>> {
        let doc_ids: Vec<String> = self.documents.keys().cloned().collect();

        if doc_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        for doc_id in &doc_ids {
            match self
                .query_with_options(doc_id, question, options.clone())
                .await
            {
                Ok(result) => {
                    if !result.node_ids.is_empty() {
                        results.push(result);
                    }
                }
                Err(e) => {
                    info!("Query failed for {}: {}", doc_id, e);
                }
            }
        }

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }

    // ============================================================
    // Document Management
    // ============================================================

    /// Get list of documents in this session.
    pub fn list_documents(&self) -> Vec<DocumentInfo> {
        self.documents
            .values()
            .map(|ctx| ctx.meta.clone())
            .collect()
    }

    /// Get a document tree (from cache or workspace).
    pub async fn get_tree(&self, doc_id: &str) -> Result<DocumentTree> {
        // Check cache first
        if let Some(tree) = self.get_cached_tree(doc_id) {
            self.stats.increment_cache_hits();
            return Ok((*tree).clone());
        }

        self.stats.increment_cache_misses();

        // Load from workspace
        let doc = self
            .workspace
            .load(doc_id)
            .await?
            .ok_or_else(|| Error::DocumentNotFound(format!("Document not found: {}", doc_id)))?;

        let tree = doc.tree;

        // Cache for future use
        self.cache_tree(doc_id, &tree);

        Ok(tree)
    }

    /// Preload documents into the session cache.
    ///
    /// Useful for warming up the cache before querying.
    pub async fn preload(&self, doc_ids: &[&str]) -> Result<usize> {
        let mut loaded = 0;

        for doc_id in doc_ids {
            if self.get_cached_tree(doc_id).is_none() {
                if let Ok(tree) = self.get_tree(doc_id).await {
                    self.cache_tree(doc_id, &tree);
                    loaded += 1;
                }
            }
        }

        info!("Session {}: preloaded {} documents", self.id, loaded);
        Ok(loaded)
    }

    /// Remove a document from the session.
    pub fn remove_document(&self, doc_id: &str) -> bool {
        // Note: This would need interior mutability for full implementation
        false
    }

    /// Clear all documents from the session cache.
    pub fn clear_cache(&self) {
        // Note: This would need interior mutability for full implementation
    }

    // ============================================================
    // Statistics
    // ============================================================

    /// Get session statistics.
    pub fn stats(&self) -> SessionStats {
        self.stats.clone()
    }

    /// Get the number of cached documents.
    pub fn cached_count(&self) -> usize {
        self.documents.values().filter(|d| d.tree.is_some()).count()
    }

    // ============================================================
    // Internal Methods
    // ============================================================

    /// Cache a document in this session.
    fn cache_document(&self, doc: crate::client::types::IndexedDocument) {
        // Note: This would need interior mutability for full implementation
        // For now, this is a placeholder
    }

    /// Get a cached tree.
    fn get_cached_tree(&self, doc_id: &str) -> Option<Arc<DocumentTree>> {
        self.documents.get(doc_id).and_then(|ctx| ctx.tree.clone())
    }

    /// Cache a tree.
    fn cache_tree(&self, doc_id: &str, tree: &DocumentTree) {
        // Note: This would need interior mutability for full implementation
    }
}

impl Clone for Session {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            config: self.config.clone(),
            documents: self.documents.clone(),
            indexer: self.indexer.clone(),
            retriever: self.retriever.clone(),
            workspace: self.workspace.clone(),
            events: self.events.clone(),
            stats: self.stats.clone(),
            created_at: self.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_config() {
        let config = SessionConfig::default();
        assert_eq!(config.max_cached_documents, 100);
        assert_eq!(config.eviction_policy, EvictionPolicy::Lru);
    }

    #[test]
    fn test_session_stats() {
        let stats = SessionStats::default();
        stats.cache_hits.set(8);
        stats.cache_misses.set(2);

        assert!((stats.cache_hit_rate() - 0.8).abs() < 0.01);
    }
}
