// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Path cache implementation for retrieval optimization.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use super::super::types::SearchPath;
use crate::config::CacheConfig as AppConfig;
use crate::document::NodeId;

/// Cache entry for a search path.
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    /// The cached value.
    value: T,
    /// When this entry was created.
    created_at: Instant,
    /// Number of times this entry has been accessed.
    access_count: usize,
}

impl<T> CacheEntry<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            created_at: Instant::now(),
            access_count: 0,
        }
    }

    fn access(&mut self) -> &T {
        self.access_count += 1;
        &self.value
    }
}

/// Configuration for the path cache.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries.
    pub max_entries: usize,
    /// Time-to-live for entries.
    pub ttl: Duration,
    /// Whether to use LRU eviction.
    pub use_lru: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self::from_app_config(&AppConfig::default())
    }
}

impl CacheConfig {
    /// Create from application config.
    pub fn from_app_config(config: &AppConfig) -> Self {
        Self {
            max_entries: config.max_entries,
            ttl: Duration::from_secs(config.ttl_secs),
            use_lru: true,
        }
    }
}

/// Path cache for retrieval optimization.
///
/// Caches search paths and node scores to avoid redundant
/// computation for similar queries.
pub struct PathCache {
    /// Cached paths by query hash.
    paths: Arc<RwLock<HashMap<u64, CacheEntry<Vec<SearchPath>>>>>,
    /// Cached node scores.
    scores: Arc<RwLock<HashMap<(u64, NodeId), CacheEntry<f32>>>>,
    /// Configuration.
    config: CacheConfig,
}

impl PathCache {
    /// Create a new path cache.
    pub fn new() -> Self {
        Self {
            paths: Arc::new(RwLock::new(HashMap::new())),
            scores: Arc::new(RwLock::new(HashMap::new())),
            config: CacheConfig::default(),
        }
    }

    /// Create a path cache with custom config.
    pub fn with_config(config: CacheConfig) -> Self {
        Self {
            paths: Arc::new(RwLock::new(HashMap::new())),
            scores: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Create from application config.
    pub fn from_app_config(config: &AppConfig) -> Self {
        Self::with_config(CacheConfig::from_app_config(config))
    }

    /// Hash a query string.
    fn hash_query(query: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        query.to_lowercase().hash(&mut hasher);
        hasher.finish()
    }

    /// Get cached paths for a query.
    pub fn get_paths(&self, query: &str) -> Option<Vec<SearchPath>> {
        let hash = Self::hash_query(query);
        let mut paths = self.paths.write().ok()?;

        if let Some(entry) = paths.get_mut(&hash) {
            // Check TTL
            if entry.created_at.elapsed() > self.config.ttl {
                paths.remove(&hash);
                return None;
            }
            return Some(entry.access().clone());
        }
        None
    }

    /// Store paths for a query.
    pub fn store_paths(&self, query: &str, paths: Vec<SearchPath>) {
        let hash = Self::hash_query(query);

        if let Ok(mut cache) = self.paths.write() {
            // Evict if at capacity
            if cache.len() >= self.config.max_entries {
                self.evict(&mut cache);
            }
            cache.insert(hash, CacheEntry::new(paths));
        }
    }

    /// Get cached score for a node.
    pub fn get_score(&self, query: &str, node_id: NodeId) -> Option<f32> {
        let hash = Self::hash_query(query);
        let mut scores = self.scores.write().ok()?;

        let key = (hash, node_id);
        if let Some(entry) = scores.get_mut(&key) {
            // Check TTL
            if entry.created_at.elapsed() > self.config.ttl {
                scores.remove(&key);
                return None;
            }
            return Some(*entry.access());
        }
        None
    }

    /// Store a node score.
    pub fn store_score(&self, query: &str, node_id: NodeId, score: f32) {
        let hash = Self::hash_query(query);

        if let Ok(mut cache) = self.scores.write() {
            let key = (hash, node_id);

            // Evict if at capacity
            if cache.len() >= self.config.max_entries {
                self.evict_scores(&mut cache);
            }
            cache.insert(key, CacheEntry::new(score));
        }
    }

    /// Evict entries using LRU or random strategy.
    fn evict<K, V>(&self, cache: &mut HashMap<K, CacheEntry<V>>)
    where
        K: std::hash::Hash + Eq + Clone,
    {
        if self.config.use_lru {
            // Find entry with lowest access count
            if let Some((min_key, _)) = cache.iter().min_by_key(|(_, e)| e.access_count) {
                let key = min_key.clone();
                cache.remove(&key);
            }
        } else {
            // Remove oldest entry
            if let Some((oldest_key, _)) = cache.iter().min_by_key(|(_, e)| e.created_at) {
                let key = oldest_key.clone();
                cache.remove(&key);
            }
        }
    }

    fn evict_scores<K, V>(&self, cache: &mut HashMap<K, CacheEntry<V>>)
    where
        K: std::hash::Hash + Eq + Clone,
    {
        self.evict(cache)
    }

    /// Clear all cached data.
    pub fn clear(&self) {
        if let Ok(mut paths) = self.paths.write() {
            paths.clear();
        }
        if let Ok(mut scores) = self.scores.write() {
            scores.clear();
        }
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let path_count = self.paths.read().map(|p| p.len()).unwrap_or(0);
        let score_count = self.scores.read().map(|s| s.len()).unwrap_or(0);

        CacheStats {
            path_entries: path_count,
            score_entries: score_count,
            max_entries: self.config.max_entries,
        }
    }
}

impl Default for PathCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Number of cached paths.
    pub path_entries: usize,
    /// Number of cached scores.
    pub score_entries: usize,
    /// Maximum entries allowed.
    pub max_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_paths() {
        let cache = PathCache::new();

        let arena = &mut indextree::Arena::new();
        let path = SearchPath::from_node(NodeId(arena.new_node(0)), 0.8);
        let paths = vec![path];

        cache.store_paths("test query", paths.clone());

        let cached = cache.get_paths("test query");
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 1);
    }

    #[test]
    fn test_cache_case_insensitive() {
        let cache = PathCache::new();

        let arena = &mut indextree::Arena::new();
        let path = SearchPath::from_node(NodeId(arena.new_node(0)), 0.8);

        let paths = vec![path];

        cache.store_paths("Test Query", paths);

        // Should find with different case
        assert!(cache.get_paths("test query").is_some());
        assert!(cache.get_paths("TEST QUERY").is_some());
    }
}
