// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Memoization store implementation.
//!
//! Provides an in-memory LRU cache with optional disk persistence.

use std::collections::HashMap;
use std::future::Future;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use chrono::{Duration, Utc};
use lru::LruCache;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, info, warn};

use super::types::{MemoEntry, MemoKey, MemoOpType, MemoStats, MemoValue};
use crate::error::Result;
use crate::utils::fingerprint::Fingerprint;

/// Default TTL for cache entries (7 days).
const DEFAULT_TTL: Duration = Duration::days(7);

/// Default maximum cache size.
const DEFAULT_MAX_SIZE: usize = 10_000;

/// Serializable format for memo store persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoStoreData {
    /// Format version.
    version: u32,

    /// Cache entries.
    entries: HashMap<String, MemoEntry>,

    /// Statistics.
    stats: MemoStats,
}

/// Atomic statistics for lock-free access.
#[derive(Debug, Default)]
struct AtomicStats {
    hits: AtomicU64,
    misses: AtomicU64,
    tokens_saved: AtomicU64,
}

impl AtomicStats {
    fn new() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            tokens_saved: AtomicU64::new(0),
        }
    }

    fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    fn add_tokens_saved(&self, tokens: u64) {
        self.tokens_saved.fetch_add(tokens, Ordering::Relaxed);
    }

    fn snapshot(&self) -> (u64, u64, u64) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
            self.tokens_saved.load(Ordering::Relaxed),
        )
    }
}

/// LLM Memoization store.
///
/// Provides caching for expensive LLM operations with:
/// - LRU eviction policy
/// - TTL-based expiration
/// - Optional disk persistence
/// - Thread-safe access
///
/// # Example
///
/// ```rust,ignore
/// let store = MemoStore::new(1000);
///
/// let summary = store.get_or_compute(
///     MemoKey::summary(&content_fp),
///     || async {
///         llm.generate_summary(content).await
///     }
/// ).await?;
/// ```
pub struct MemoStore {
    /// LRU cache for entries.
    cache: Arc<RwLock<LruCache<String, MemoEntry>>>,

    /// Statistics (async for safe updates).
    stats: Arc<AsyncRwLock<MemoStats>>,

    /// TTL for entries.
    ttl: Duration,

    /// Model identifier for cache keys.
    model_id: Option<String>,

    /// Version for cache invalidation.
    version: u32,
}

impl std::fmt::Debug for MemoStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoStore")
            .field("ttl", &self.ttl)
            .field("model_id", &self.model_id)
            .field("version", &self.version)
            .field("cache_len", &self.cache.read().len())
            .finish()
    }
}

impl Clone for MemoStore {
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
            stats: Arc::clone(&self.stats),
            ttl: self.ttl,
            model_id: self.model_id.clone(),
            version: self.version,
        }
    }
}

impl MemoStore {
    /// Create a new memo store with default size.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_MAX_SIZE)
    }

    /// Create a new memo store with specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(
                std::num::NonZeroUsize::new(capacity).unwrap_or(std::num::NonZeroUsize::new(1000).unwrap()),
            ))),
            stats: Arc::new(AsyncRwLock::new(MemoStats::default())),
            ttl: DEFAULT_TTL,
            model_id: None,
            version: 1,
        }
    }

    /// Set the TTL for cache entries.
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Set the model identifier.
    pub fn with_model(mut self, model_id: &str) -> Self {
        self.model_id = Some(model_id.to_string());
        self
    }

    /// Set the version.
    pub fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Get a cached value if present and not expired.
    pub fn get(&self, key: &MemoKey) -> Option<MemoValue> {
        let full_key = self.make_key(key);
        let mut cache = self.cache.write();

        if let Some(entry) = cache.get_mut(&full_key) {
            // Check TTL
            if entry.is_expired(self.ttl) {
                cache.pop(&full_key);
                return None;
            }

            // Record hit
            entry.record_hit();
            debug!("Memo cache hit for {:?}", key.op_type);
            return Some(entry.value.clone());
        }

        None
    }

    /// Put a value in the cache.
    pub fn put(&self, key: MemoKey, value: MemoValue) {
        self.put_with_tokens(key, value, 0);
    }

    /// Put a value in the cache with token count.
    pub fn put_with_tokens(&self, key: MemoKey, value: MemoValue, tokens_saved: u64) {
        let full_key = self.make_key(&key);
        let entry = MemoEntry::with_tokens(value, tokens_saved);

        let mut cache = self.cache.write();
        cache.put(full_key, entry);

        debug!("Memo cache put for {:?}", key.op_type);
    }

    /// Get a value or compute it if not present.
    ///
    /// This is the primary method for using the memo store.
    /// It will return the cached value if present, or call the
    /// provided compute function and cache the result.
    pub async fn get_or_compute<F, Fut>(
        &self,
        key: MemoKey,
        compute: F,
    ) -> Result<MemoValue>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(MemoValue, u64)>>, // (value, tokens)
    {
        // Check cache first (synchronous)
        if let Some(value) = self.get(&key) {
            // Update stats
            let mut stats = self.stats.write().await;
            stats.hits += 1;
            return Ok(value);
        }

        // Record miss
        {
            let mut stats = self.stats.write().await;
            stats.misses += 1;
        }

        // Compute
        let (value, tokens) = compute().await?;

        // Cache result
        self.put_with_tokens(key.clone(), value.clone(), tokens);

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.tokens_saved += tokens;
        }

        Ok(value)
    }

    /// Check if a key exists in the cache.
    pub fn contains(&self, key: &MemoKey) -> bool {
        let full_key = self.make_key(key);
        let cache = self.cache.read();
        cache.contains(&full_key)
    }

    /// Remove a key from the cache.
    pub fn remove(&self, key: &MemoKey) -> Option<MemoValue> {
        let full_key = self.make_key(key);
        let mut cache = self.cache.write();
        cache.pop(&full_key).map(|e| e.value)
    }

    /// Clear all entries from the cache.
    pub fn clear(&self) {
        let mut cache = self.cache.write();
        cache.clear();
        debug!("Memo cache cleared");
    }

    /// Get the number of entries in the cache.
    pub fn len(&self) -> usize {
        let cache = self.cache.read();
        cache.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get cache statistics.
    pub async fn stats(&self) -> MemoStats {
        let stats = self.stats.read().await;
        let mut result = stats.clone();
        result.entries = self.len();
        result
    }

    /// Get cache statistics synchronously.
    ///
    /// This acquires a read lock on the stats, which is generally fast.
    /// Use this when you need stats without async context.
    pub fn stats_snapshot(&self) -> MemoStats {
        // Use try_read to avoid blocking; fall back to defaults if locked
        match self.stats.try_read() {
            Ok(stats) => {
                let mut result = stats.clone();
                result.entries = self.len();
                result
            }
            Err(_) => MemoStats {
                entries: self.len(),
                ..Default::default()
            },
        }
    }

    /// Invalidate all entries of a specific operation type.
    ///
    /// This is useful for batch invalidation when the algorithm for
    /// a specific operation type changes.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Invalidate all pilot decision caches
    /// let removed = store.invalidate_by_op_type(MemoOpType::PilotDecision);
    /// println!("Removed {} cached pilot decisions", removed);
    /// ```
    pub fn invalidate_by_op_type(&self, op_type: MemoOpType) -> usize {
        let mut cache = self.cache.write();
        let before = cache.len();

        // Collect keys to remove based on entry value type
        let keys_to_remove: Vec<String> = cache
            .iter()
            .filter_map(|(key, entry)| {
                let matches = match (&entry.value, op_type) {
                    (MemoValue::Summary(_), MemoOpType::Summary) => true,
                    (MemoValue::PilotDecision(_), MemoOpType::PilotDecision) => true,
                    (MemoValue::QueryAnalysis(_), MemoOpType::QueryAnalysis) => true,
                    (MemoValue::Extraction(_), MemoOpType::Extraction) => true,
                    _ => false,
                };
                if matches { Some(key.clone()) } else { None }
            })
            .collect();

        // Remove entries
        for key in keys_to_remove {
            cache.pop(&key);
        }

        let removed = before - cache.len();
        if removed > 0 {
            debug!("Invalidated {} entries for op_type {:?}", removed, op_type);
        }
        removed
    }

    /// Invalidate all entries matching a model ID prefix.
    ///
    /// This is useful when switching models or when a model's behavior changes.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Invalidate all GPT-4 caches
    /// let removed = store.invalidate_by_model_prefix("gpt-4");
    /// ```
    pub fn invalidate_by_model_prefix(&self, prefix: &str) -> usize {
        let mut cache = self.cache.write();
        let before = cache.len();

        // Since the key is a fingerprint, we need to check model_id from entries
        // For now, we'll clear all entries if prefix matches our model_id
        // A better approach would be to store model_id in entry metadata
        let should_clear = self.model_id.as_ref()
            .map(|m| m.starts_with(prefix))
            .unwrap_or(false);

        if should_clear {
            cache.clear();
            let removed = before;
            debug!("Invalidated all {} entries (model prefix '{}')", removed, prefix);
            return removed;
        }

        0
    }

    /// Remove expired entries.
    pub fn prune_expired(&self) -> usize {
        let mut cache = self.cache.write();
        let before = cache.len();

        // Collect expired keys
        let expired: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| entry.is_expired(self.ttl))
            .map(|(k, _)| k.clone())
            .collect();

        // Remove expired entries
        for key in expired {
            cache.pop(&key);
        }

        let removed = before - cache.len();
        if removed > 0 {
            debug!("Pruned {} expired memo entries", removed);
        }
        removed
    }

    /// Save the cache to disk.
    pub async fn save(&self, path: &Path) -> Result<()> {
        let cache = self.cache.read();
        let stats = self.stats.read().await;

        let entries: HashMap<String, MemoEntry> = cache
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let data = MemoStoreData {
            version: 1,
            entries,
            stats: stats.clone(),
        };

        let parent = path.parent().ok_or_else(|| {
            crate::Error::Parse("Invalid path for memo store".to_string())
        })?;
        tokio::fs::create_dir_all(parent).await?;

        let temp_path = path.with_extension("tmp");
        let json = serde_json::to_vec_pretty(&data)
            .map_err(|e| crate::Error::Parse(format!("Failed to serialize memo store: {}", e)))?;
        tokio::fs::write(&temp_path, &json).await?;
        tokio::fs::rename(&temp_path, path).await?;

        info!("Saved memo store with {} entries to {:?}", data.entries.len(), path);
        Ok(())
    }

    /// Load the cache from disk.
    pub async fn load(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let bytes = tokio::fs::read(path).await?;
        let data: MemoStoreData = serde_json::from_slice(&bytes)
            .map_err(|e| crate::Error::Parse(format!("Failed to deserialize memo store: {}", e)))?;

        let mut cache = self.cache.write();
        let mut stats = self.stats.write().await;

        for (key, entry) in data.entries {
            // Skip expired entries
            if !entry.is_expired(self.ttl) {
                cache.put(key, entry);
            }
        }

        stats.entries = cache.len();
        stats.hits = data.stats.hits;
        stats.misses = data.stats.misses;
        stats.tokens_saved = data.stats.tokens_saved;
        stats.cost_saved = data.stats.cost_saved;

        info!("Loaded memo store with {} entries from {:?}", cache.len(), path);
        Ok(())
    }

    /// Make a full cache key from a MemoKey.
    fn make_key(&self, key: &MemoKey) -> String {
        // Include model_id and version in the key
        let mut key_with_context = key.clone();
        if key_with_context.model_id.is_none() {
            key_with_context.model_id = self.model_id.clone();
        }
        if key_with_context.version == 0 {
            key_with_context.version = self.version;
        }
        key_with_context.fingerprint().to_string()
    }
}

impl Default for MemoStore {
    fn default() -> Self {
        Self::new()
    }
}

/// A helper for building memo keys with context.
pub struct MemoKeyBuilder {
    model_id: Option<String>,
    version: u32,
}

impl MemoKeyBuilder {
    /// Create a new key builder.
    pub fn new() -> Self {
        Self {
            model_id: None,
            version: 1,
        }
    }

    /// Set the model identifier.
    pub fn with_model(mut self, model_id: &str) -> Self {
        self.model_id = Some(model_id.to_string());
        self
    }

    /// Set the version.
    pub fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Build a summary key.
    pub fn summary_key(&self, content_fp: &Fingerprint) -> MemoKey {
        MemoKey {
            op_type: super::types::MemoOpType::Summary,
            input_fp: *content_fp,
            model_id: self.model_id.clone(),
            version: self.version,
            context_fp: Fingerprint::zero(),
        }
    }

    /// Build a pilot decision key.
    pub fn pilot_key(&self, context_fp: &Fingerprint, query_fp: &Fingerprint) -> MemoKey {
        MemoKey {
            op_type: super::types::MemoOpType::PilotDecision,
            input_fp: *query_fp,
            model_id: self.model_id.clone(),
            version: self.version,
            context_fp: *context_fp,
        }
    }

    /// Build a query analysis key.
    pub fn query_analysis_key(&self, query_fp: &Fingerprint) -> MemoKey {
        MemoKey {
            op_type: super::types::MemoOpType::QueryAnalysis,
            input_fp: *query_fp,
            model_id: self.model_id.clone(),
            version: self.version,
            context_fp: Fingerprint::zero(),
        }
    }

    /// Build an extraction key.
    pub fn extraction_key(&self, content_fp: &Fingerprint) -> MemoKey {
        MemoKey {
            op_type: super::types::MemoOpType::Extraction,
            input_fp: *content_fp,
            model_id: self.model_id.clone(),
            version: self.version,
            context_fp: Fingerprint::zero(),
        }
    }
}

impl Default for MemoKeyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_test_key() -> MemoKey {
        let fp = Fingerprint::from_str("test content");
        MemoKey::summary(&fp)
    }

    #[test]
    fn test_memo_store_basic() {
        let store = MemoStore::new();
        let key = make_test_key();

        assert!(!store.contains(&key));

        store.put(key.clone(), MemoValue::Summary("Test summary".to_string()));

        assert!(store.contains(&key));

        let value = store.get(&key);
        assert!(value.is_some());
        assert_eq!(value.unwrap().as_summary(), Some("Test summary"));
    }

    #[test]
    fn test_memo_store_lru_eviction() {
        let store = MemoStore::with_capacity(3);

        for i in 0..5 {
            let fp = Fingerprint::from_str(&format!("content {}", i));
            let key = MemoKey::summary(&fp);
            store.put(key, MemoValue::Summary(format!("Summary {}", i)));
        }

        // Only 3 entries should remain
        assert_eq!(store.len(), 3);
    }

    #[tokio::test]
    async fn test_memo_store_get_or_compute() {
        let store = MemoStore::new();
        let key = make_test_key();

        let call_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let count_clone = call_count.clone();

        // First call should compute
        let result = store
            .get_or_compute(key.clone(), || {
                let c = count_clone.clone();
                async move {
                    c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok((MemoValue::Summary("Computed".to_string()), 100))
                }
            })
            .await
            .unwrap();

        assert_eq!(result.as_summary(), Some("Computed"));
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Second call should use cache
        let result2 = store
            .get_or_compute(key.clone(), || {
                let c = count_clone.clone();
                async move {
                    c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok((MemoValue::Summary("Should not be called".to_string()), 100))
                }
            })
            .await
            .unwrap();

        assert_eq!(result2.as_summary(), Some("Computed"));
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1); // Still 1
    }

    #[tokio::test]
    async fn test_memo_store_persistence() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("memo.json");

        let store = MemoStore::new();
        let key = make_test_key();

        store.put_with_tokens(key.clone(), MemoValue::Summary("Test summary".to_string()), 100);

        // Save
        store.save(&path).await.unwrap();
        assert!(path.exists());

        // Load into new store
        let store2 = MemoStore::new();
        store2.load(&path).await.unwrap();

        assert!(store2.contains(&key));
        let value = store2.get(&key);
        assert_eq!(value.unwrap().as_summary(), Some("Test summary"));
    }

    #[tokio::test]
    async fn test_memo_store_stats() {
        let store = MemoStore::new();
        let key = make_test_key();

        // Miss
        store
            .get_or_compute(key.clone(), || async {
                Ok((MemoValue::Summary("Test".to_string()), 100))
            })
            .await
            .unwrap();

        // Hit via get_or_compute (this updates global stats)
        store
            .get_or_compute(key.clone(), || async {
                Ok((MemoValue::Summary("Should not be called".to_string()), 0))
            })
            .await
            .unwrap();

        let stats = store.stats().await;
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.tokens_saved, 100);
    }

    #[test]
    fn test_memo_key_builder() {
        let builder = MemoKeyBuilder::new().with_model("gpt-4").with_version(2);

        let fp = Fingerprint::from_str("content");
        let key = builder.summary_key(&fp);

        assert_eq!(key.model_id, Some("gpt-4".to_string()));
        assert_eq!(key.version, 2);
    }
}
