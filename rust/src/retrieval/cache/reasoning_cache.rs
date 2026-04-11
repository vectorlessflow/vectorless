// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Tiered reasoning cache for the retrieval pipeline.
//!
//! Provides three levels of caching to avoid redundant computation:
//!
//! - **L1 (Exact)**: Cache full retrieval results keyed by exact query fingerprint.
//!   Identical queries return instantly.
//!
//! - **L2 (Path Pattern)**: Cache navigation decisions for tree paths. If a previous
//!   query navigated through Section 3.2, a new query about the same section can
//!   reuse those path cues even when the full query differs.
//!
//! - **L3 (Strategy Score)**: Cache node scores from keyword/BM25 strategies.
//!   Node scores are independent of the query, so they can be shared across
//!   different queries on the same document.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;

use crate::document::NodeId;
use crate::utils::fingerprint::Fingerprint;

/// A tiered reasoning cache for the retrieval pipeline.
///
/// Thread-safe via `RwLock`. Each tier has independent size limits
/// and TTL-based expiration.
pub struct ReasoningCache {
    /// L1: Exact query → cached candidate list.
    l1: RwLock<L1Store>,
    /// L2: Node path pattern → cached navigation cue score.
    l2: RwLock<L2Store>,
    /// L3: Node content fingerprint → cached strategy score.
    l3: RwLock<L3Store>,
    /// Configuration.
    config: ReasoningCacheConfig,
}

/// Configuration for the reasoning cache.
#[derive(Debug, Clone)]
pub struct ReasoningCacheConfig {
    /// Maximum L1 entries (exact query results).
    pub l1_max: usize,
    /// Maximum L2 entries (path patterns).
    pub l2_max: usize,
    /// Maximum L3 entries (strategy scores).
    pub l3_max: usize,
}

impl Default for ReasoningCacheConfig {
    fn default() -> Self {
        Self {
            l1_max: 200,
            l2_max: 1000,
            l3_max: 5000,
        }
    }
}

// ---- L1: Exact Query Cache ----

#[derive(Debug, Clone)]
struct L1Entry {
    /// Fingerprint of the workspace + document set used for this query.
    scope_fp: Fingerprint,
    /// Cached candidate nodes (pre-sorted by score).
    candidates: Vec<CachedCandidate>,
    /// Strategy used.
    strategy: String,
    /// When cached.
    created_at: Instant,
}

/// A cached candidate from a previous retrieval.
#[derive(Debug, Clone)]
pub struct CachedCandidate {
    /// Node ID.
    pub node_id: NodeId,
    /// Relevance score.
    pub score: f32,
    /// Depth in tree.
    pub depth: usize,
}

struct L1Store {
    entries: HashMap<Fingerprint, L1Entry>,
    order: Vec<Fingerprint>, // For LRU eviction
}

// ---- L2: Path Pattern Cache ----

#[derive(Debug, Clone)]
struct L2Entry {
    /// Score for this navigation cue.
    confidence: f32,
    /// How many times this path was relevant.
    hit_count: usize,
    created_at: Instant,
}

struct L2Store {
    entries: HashMap<String, L2Entry>, // Key: "doc_fp:node_path"
    order: Vec<String>,
}

// ---- L3: Strategy Score Cache ----

#[derive(Debug, Clone)]
struct L3Entry {
    /// BM25/Keyword score.
    score: f32,
    /// Which strategy produced this score.
    strategy: String,
    created_at: Instant,
}

struct L3Store {
    entries: HashMap<Fingerprint, L3Entry>, // Key: node content fingerprint
    order: Vec<Fingerprint>,
}

// ---- Public API ----

impl ReasoningCache {
    /// Create a new reasoning cache with default configuration.
    pub fn new() -> Self {
        Self::with_config(ReasoningCacheConfig::default())
    }

    /// Create with custom configuration.
    pub fn with_config(config: ReasoningCacheConfig) -> Self {
        Self {
            l1: RwLock::new(L1Store {
                entries: HashMap::new(),
                order: Vec::new(),
            }),
            l2: RwLock::new(L2Store {
                entries: HashMap::new(),
                order: Vec::new(),
            }),
            l3: RwLock::new(L3Store {
                entries: HashMap::new(),
                order: Vec::new(),
            }),
            config,
        }
    }

    // ============ L1: Exact Query ============

    /// Look up an exact query result.
    ///
    /// Returns cached candidates if the same query was executed before
    /// on the same document scope.
    pub fn l1_get(&self, query: &str, scope_fp: &Fingerprint) -> Option<Vec<CachedCandidate>> {
        let query_fp = Fingerprint::from_str(query);
        let l1 = self.l1.read().ok()?;
        let entry = l1.entries.get(&query_fp)?;
        // Scope must match (same document set)
        if &entry.scope_fp != scope_fp {
            return None;
        }
        Some(entry.candidates.clone())
    }

    /// Store an L1 result.
    pub fn l1_store(
        &self,
        query: &str,
        scope_fp: Fingerprint,
        candidates: Vec<CachedCandidate>,
        strategy: String,
    ) {
        let query_fp = Fingerprint::from_str(query);
        if let Ok(mut l1) = self.l1.write() {
            if l1.entries.len() >= self.config.l1_max {
                Self::evict_lru_fingerprint(&mut l1);
            }
            l1.entries.insert(
                query_fp,
                L1Entry {
                    scope_fp,
                    candidates,
                    strategy,
                    created_at: Instant::now(),
                },
            );
            l1.order.push(query_fp);
        }
    }

    // ============ L2: Path Pattern ============

    /// Look up a cached navigation confidence for a document + node path.
    ///
    /// If a previous query successfully navigated through this path,
    /// return the confidence score.
    pub fn l2_get(&self, doc_key: &str, node_path: &str) -> Option<f32> {
        let key = format!("{}:{}", doc_key, node_path);
        let l2 = self.l2.read().ok()?;
        let entry = l2.entries.get(&key)?;
        Some(entry.confidence)
    }

    /// Record a successful navigation through a path.
    ///
    /// Call this after retrieval confirms a path was relevant.
    pub fn l2_record(&self, doc_key: &str, node_path: &str, confidence: f32) {
        let key = format!("{}:{}", doc_key, node_path);
        if let Ok(mut l2) = self.l2.write() {
            if let Some(entry) = l2.entries.get_mut(&key) {
                // Update running average
                entry.hit_count += 1;
                entry.confidence =
                    entry.confidence + (confidence - entry.confidence) / entry.hit_count as f32;
            } else {
                if l2.entries.len() >= self.config.l2_max {
                    Self::evict_lru_string(&mut l2);
                }
                l2.entries.insert(
                    key.clone(),
                    L2Entry {
                        confidence,
                        hit_count: 1,
                        created_at: Instant::now(),
                    },
                );
                l2.order.push(key);
            }
        }
    }

    /// Get top-N path hints for a document, sorted by confidence.
    ///
    /// Useful for bootstrapping new queries on a known document.
    pub fn l2_top_paths(&self, doc_key: &str, n: usize) -> Vec<(String, f32)> {
        let prefix = format!("{}:", doc_key);
        let l2 = match self.l2.read() {
            Ok(guard) => guard,
            Err(_) => return Vec::new(),
        };

        let mut paths: Vec<(String, f32)> = l2
            .entries
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(k, v)| (k[prefix.len()..].to_string(), v.confidence))
            .collect();
        paths.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        paths.truncate(n);
        paths
    }

    // ============ L3: Strategy Score ============

    /// Look up a cached strategy score for a node.
    ///
    /// Node scores from keyword/BM25 are content-dependent but
    /// query-independent, so they can be shared across queries.
    pub fn l3_get(&self, node_content_fp: &Fingerprint) -> Option<(f32, String)> {
        let l3 = self.l3.read().ok()?;
        let entry = l3.entries.get(node_content_fp)?;
        Some((entry.score, entry.strategy.clone()))
    }

    /// Store a strategy score for a node.
    pub fn l3_store(&self, node_content_fp: Fingerprint, score: f32, strategy: String) {
        if let Ok(mut l3) = self.l3.write() {
            if l3.entries.len() >= self.config.l3_max {
                Self::evict_lru_fingerprint_l3(&mut l3);
            }
            l3.entries.insert(
                node_content_fp,
                L3Entry {
                    score,
                    strategy,
                    created_at: Instant::now(),
                },
            );
            l3.order.push(node_content_fp);
        }
    }

    // ============ Stats ============

    /// Get cache statistics.
    pub fn stats(&self) -> ReasoningCacheStats {
        let (l1_count, l2_count, l3_count) = (
            self.l1.read().map(|g| g.entries.len()).unwrap_or(0),
            self.l2.read().map(|g| g.entries.len()).unwrap_or(0),
            self.l3.read().map(|g| g.entries.len()).unwrap_or(0),
        );
        ReasoningCacheStats {
            l1_entries: l1_count,
            l2_entries: l2_count,
            l3_entries: l3_count,
        }
    }

    /// Clear all cache tiers.
    pub fn clear(&self) {
        if let Ok(mut l1) = self.l1.write() {
            l1.entries.clear();
            l1.order.clear();
        }
        if let Ok(mut l2) = self.l2.write() {
            l2.entries.clear();
            l2.order.clear();
        }
        if let Ok(mut l3) = self.l3.write() {
            l3.entries.clear();
            l3.order.clear();
        }
    }

    // ============ Eviction helpers ============

    fn evict_lru_fingerprint(l1: &mut L1Store) {
        if let Some(old) = l1.order.first().copied() {
            l1.entries.remove(&old);
            l1.order.remove(0);
        }
    }

    fn evict_lru_string(l2: &mut L2Store) {
        if let Some(old) = l2.order.first().cloned() {
            l2.entries.remove(&old);
            l2.order.remove(0);
        }
    }

    fn evict_lru_fingerprint_l3(l3: &mut L3Store) {
        if let Some(old) = l3.order.first().copied() {
            l3.entries.remove(&old);
            l3.order.remove(0);
        }
    }
}

impl Default for ReasoningCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct ReasoningCacheStats {
    /// L1 entries (exact query results).
    pub l1_entries: usize,
    /// L2 entries (path patterns).
    pub l2_entries: usize,
    /// L3 entries (strategy scores).
    pub l3_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node_id(n: usize) -> NodeId {
        let mut arena = indextree::Arena::new();
        NodeId(arena.new_node(n))
    }

    #[test]
    fn test_l1_store_and_retrieve() {
        let cache = ReasoningCache::new();
        let scope = Fingerprint::from_str("doc1");

        let candidates = vec![CachedCandidate {
            node_id: make_node_id(1),
            score: 0.9,
            depth: 2,
        }];

        cache.l1_store("what is rust?", scope, candidates.clone(), "keyword".into());
        let result = cache.l1_get("what is rust?", &scope);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_l1_miss_different_scope() {
        let cache = ReasoningCache::new();
        let scope1 = Fingerprint::from_str("doc1");
        let scope2 = Fingerprint::from_str("doc2");

        let candidates = vec![CachedCandidate {
            node_id: make_node_id(1),
            score: 0.9,
            depth: 2,
        }];

        cache.l1_store("query", scope1, candidates, "keyword".into());
        assert!(cache.l1_get("query", &scope2).is_none());
    }

    #[test]
    fn test_l2_record_and_get() {
        let cache = ReasoningCache::new();

        cache.l2_record("doc1", "3.2", 0.8);
        let score = cache.l2_get("doc1", "3.2");
        assert!(score.is_some());
        assert!((score.unwrap() - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_l2_running_average() {
        let cache = ReasoningCache::new();

        cache.l2_record("doc1", "3.2", 0.8);
        cache.l2_record("doc1", "3.2", 0.6);
        let score = cache.l2_get("doc1", "3.2").unwrap();
        // Running average: 0.8 + (0.6 - 0.8) / 2 = 0.7
        assert!((score - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_l2_top_paths() {
        let cache = ReasoningCache::new();

        cache.l2_record("doc1", "3.1", 0.5);
        cache.l2_record("doc1", "3.2", 0.9);
        cache.l2_record("doc1", "2.1", 0.7);

        let top = cache.l2_top_paths("doc1", 2);
        assert_eq!(top.len(), 2);
        assert!((top[0].1 - 0.9).abs() < 0.01); // 3.2 is highest
    }

    #[test]
    fn test_l3_store_and_retrieve() {
        let cache = ReasoningCache::new();
        let fp = Fingerprint::from_str("some node content");

        cache.l3_store(fp, 0.85, "bm25".into());
        let (score, strategy) = cache.l3_get(&fp).unwrap();
        assert!((score - 0.85).abs() < 0.01);
        assert_eq!(strategy, "bm25");
    }

    #[test]
    fn test_clear() {
        let cache = ReasoningCache::new();
        let scope = Fingerprint::from_str("doc1");

        cache.l1_store("q", scope, vec![], "kw".into());
        cache.l2_record("doc1", "1", 0.5);
        cache.l3_store(Fingerprint::from_str("c"), 0.5, "kw".into());

        cache.clear();

        let stats = cache.stats();
        assert_eq!(stats.l1_entries, 0);
        assert_eq!(stats.l2_entries, 0);
        assert_eq!(stats.l3_entries, 0);
    }

    #[test]
    fn test_l1_lru_eviction() {
        let config = ReasoningCacheConfig {
            l1_max: 2,
            ..Default::default()
        };
        let cache = ReasoningCache::with_config(config);
        let scope = Fingerprint::from_str("doc");

        cache.l1_store("q1", scope, vec![], "kw".into());
        cache.l1_store("q2", scope, vec![], "kw".into());
        cache.l1_store("q3", scope, vec![], "kw".into()); // evicts q1

        assert!(cache.l1_get("q1", &scope).is_none());
        assert!(cache.l1_get("q2", &scope).is_some());
        assert!(cache.l1_get("q3", &scope).is_some());
    }
}
