// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document cache with LRU eviction policy.
//!
//! This module provides a thread-safe LRU cache for loaded documents,
//! allowing efficient reuse of loaded document data while limiting memory usage.
//!
//! # Metrics
//!
//! The cache tracks:
//! - Hits: Number of successful cache lookups
//! - Misses: Number of failed cache lookups
//! - Evictions: Number of entries evicted due to capacity
//! - Utilization: Current usage as percentage of capacity

use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use lru::LruCache;

use super::persistence::PersistedDocument;
use vectorless_error::Error;
use vectorless_error::Result;

/// Default cache size (number of documents).
const DEFAULT_CACHE_SIZE: usize = 100;

/// A thread-safe LRU cache for documents.
///
/// Uses interior mutability via `Mutex` for safe concurrent access.
/// The cache automatically evicts least-recently-used entries when full.
///
/// # Metrics
///
/// The cache maintains atomic counters for:
/// - **hits**: Successful cache lookups
/// - **misses**: Failed cache lookups (document not in cache)
/// - **evictions**: Entries removed due to capacity limits
#[derive(Debug)]
pub struct DocumentCache {
    /// Inner cache protected by Mutex.
    inner: Mutex<LruCache<String, PersistedDocument>>,
    /// Maximum capacity.
    capacity: usize,
    /// Number of cache hits.
    hits: AtomicU64,
    /// Number of cache misses.
    misses: AtomicU64,
    /// Number of cache evictions.
    evictions: AtomicU64,
}

impl DocumentCache {
    /// Create a new cache with default capacity (100 documents).
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CACHE_SIZE)
    }

    /// Create a new cache with custom capacity.
    ///
    /// # Panics
    ///
    /// This function does not panic, but capacities below 1 are normalized to 1.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        let non_zero = NonZeroUsize::new(capacity)
            .unwrap_or_else(|| NonZeroUsize::new(DEFAULT_CACHE_SIZE).expect("default is non-zero"));

        Self {
            inner: Mutex::new(LruCache::new(non_zero)),
            capacity,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }

    /// Get a document from the cache.
    ///
    /// Returns `None` if the document is not in the cache.
    /// Updates the access order (moves to most-recently-used).
    ///
    /// # Errors
    ///
    /// Returns an error if the cache lock is poisoned.
    pub fn get(&self, id: &str) -> Result<Option<PersistedDocument>> {
        let mut cache = self.lock()?;
        let result = cache.get(id).cloned();

        // Update metrics
        if result.is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }

        Ok(result)
    }

    /// Check if a document is in the cache.
    pub fn contains(&self, id: &str) -> bool {
        self.lock().map(|cache| cache.contains(id)).unwrap_or(false)
    }

    /// Put a document into the cache.
    ///
    /// If the cache is full, evicts the least-recently-used entry.
    /// Returns the evicted entry if any.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache lock is poisoned.
    pub fn put(&self, id: String, doc: PersistedDocument) -> Result<Option<PersistedDocument>> {
        let mut cache = self.lock()?;

        // Track capacity before put to detect eviction
        let was_full = cache.len() >= self.capacity;

        let evicted = cache.put(id, doc);

        // Track evictions
        if evicted.is_some() || was_full {
            self.evictions.fetch_add(1, Ordering::Relaxed);
        }

        Ok(evicted)
    }

    /// Remove a document from the cache.
    ///
    /// Returns the removed document if it was in the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache lock is poisoned.
    pub fn remove(&self, id: &str) -> Result<Option<PersistedDocument>> {
        let mut cache = self.lock()?;
        Ok(cache.pop(id))
    }

    /// Clear all entries from the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache lock is poisoned.
    pub fn clear(&self) -> Result<()> {
        let mut cache = self.lock()?;
        cache.clear();
        Ok(())
    }

    /// Get the number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.lock().map(|cache| cache.len()).unwrap_or(0)
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the maximum capacity of the cache.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get cache utilization (0.0 to 1.0).
    pub fn utilization(&self) -> f64 {
        let len = self.len();
        if self.capacity == 0 {
            return 0.0;
        }
        len as f64 / self.capacity as f64
    }

    /// Get all document IDs currently in the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache lock is poisoned.
    pub fn keys(&self) -> Result<Vec<String>> {
        let cache = self.lock()?;
        Ok(cache.iter().map(|(k, _)| k.clone()).collect())
    }

    /// Get cache statistics including metrics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            len: self.len(),
            capacity: self.capacity,
            utilization: self.utilization(),
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
        }
    }

    /// Get the number of cache hits.
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Get the number of cache misses.
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }

    /// Get the number of cache evictions.
    pub fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::Relaxed)
    }

    /// Get the cache hit rate (0.0 to 1.0).
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// Reset all metrics counters to zero.
    pub fn reset_metrics(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
    }

    /// Lock the inner cache.
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, LruCache<String, PersistedDocument>>> {
        self.inner
            .lock()
            .map_err(|_| Error::Cache("Cache lock poisoned".to_string()))
    }
}

impl Default for DocumentCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics including metrics.
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    /// Number of entries in cache.
    pub len: usize,
    /// Maximum capacity.
    pub capacity: usize,
    /// Utilization (0.0 to 1.0).
    pub utilization: f64,
    /// Number of cache hits.
    pub hits: u64,
    /// Number of cache misses.
    pub misses: u64,
    /// Number of cache evictions.
    pub evictions: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorless_document::DocumentTree;
    use crate::storage::{DocumentMeta, PersistedDocument};

    fn create_test_doc(id: &str) -> PersistedDocument {
        let meta = DocumentMeta::new(id, "Test Doc", "md");
        let tree = DocumentTree::new("Root", "Content");
        PersistedDocument::new(meta, tree)
    }

    #[test]
    fn test_cache_basic() {
        let cache = DocumentCache::with_capacity(3);

        // Add documents
        let doc1 = create_test_doc("doc1");
        let doc2 = create_test_doc("doc2");

        cache.put("doc1".to_string(), doc1.clone()).unwrap();
        cache.put("doc2".to_string(), doc2.clone()).unwrap();

        assert_eq!(cache.len(), 2);
        assert!(cache.contains("doc1"));
        assert!(cache.contains("doc2"));
    }

    #[test]
    fn test_cache_get() {
        let cache = DocumentCache::with_capacity(3);
        let doc = create_test_doc("doc1");

        cache.put("doc1".to_string(), doc).unwrap();

        let retrieved = cache.get("doc1").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().meta.id, "doc1");

        let missing = cache.get("missing").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_cache_eviction() {
        let cache = DocumentCache::with_capacity(2);

        cache
            .put("doc1".to_string(), create_test_doc("doc1"))
            .unwrap();
        cache
            .put("doc2".to_string(), create_test_doc("doc2"))
            .unwrap();
        cache
            .put("doc3".to_string(), create_test_doc("doc3"))
            .unwrap();

        // doc1 should be evicted (least recently used)
        assert!(!cache.contains("doc1"));
        assert!(cache.contains("doc2"));
        assert!(cache.contains("doc3"));
    }

    #[test]
    fn test_cache_remove() {
        let cache = DocumentCache::new();

        cache
            .put("doc1".to_string(), create_test_doc("doc1"))
            .unwrap();
        assert!(cache.contains("doc1"));

        let removed = cache.remove("doc1").unwrap();
        assert!(removed.is_some());
        assert!(!cache.contains("doc1"));

        let not_found = cache.remove("missing").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_cache_clear() {
        let cache = DocumentCache::new();

        cache
            .put("doc1".to_string(), create_test_doc("doc1"))
            .unwrap();
        cache
            .put("doc2".to_string(), create_test_doc("doc2"))
            .unwrap();

        assert_eq!(cache.len(), 2);

        cache.clear().unwrap();

        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_utilization() {
        let cache = DocumentCache::with_capacity(10);

        assert_eq!(cache.utilization(), 0.0);

        cache
            .put("doc1".to_string(), create_test_doc("doc1"))
            .unwrap();
        assert!((cache.utilization() - 0.1).abs() < 0.01);

        cache
            .put("doc2".to_string(), create_test_doc("doc2"))
            .unwrap();
        assert!((cache.utilization() - 0.2).abs() < 0.01);
    }
}
