// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval metrics collection.

use std::sync::atomic::{AtomicU64, Ordering};

use vectorless_config::RetrievalMetricsConfig;

/// Retrieval metrics tracker.
#[derive(Debug, Default)]
pub struct RetrievalMetrics {
    /// Total number of queries.
    pub total_queries: AtomicU64,
    /// Total number of search iterations.
    pub total_iterations: AtomicU64,
    /// Sum of iterations (for average).
    pub iterations_sum: AtomicU64,
    /// Total number of nodes visited.
    pub nodes_visited: AtomicU64,
    /// Total number of paths found.
    pub paths_found: AtomicU64,
    /// Sum of path lengths (for average).
    pub path_length_sum: AtomicU64,
    /// Sum of path scores stored as scaled integer (multiply by 1_000_000 for actual value).
    pub path_score_sum_scaled: AtomicU64,
    /// Number of paths with score >= 0.5.
    pub high_score_paths: AtomicU64,
    /// Number of paths with score < 0.3.
    pub low_score_paths: AtomicU64,
    /// Number of cache hits.
    pub cache_hits: AtomicU64,
    /// Number of cache misses.
    pub cache_misses: AtomicU64,
    /// Total latency in milliseconds.
    pub total_latency_ms: AtomicU64,
    /// Number of backtracks.
    pub backtracks: AtomicU64,
    /// Number of sufficiency checks.
    pub sufficiency_checks: AtomicU64,
    /// Number of times content was sufficient.
    pub sufficient_results: AtomicU64,
}

impl RetrievalMetrics {
    /// Create new retrieval metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a query.
    pub fn record_query(
        &self,
        iterations: u64,
        nodes: u64,
        latency_ms: u64,
        config: &RetrievalMetricsConfig,
    ) {
        self.total_queries.fetch_add(1, Ordering::Relaxed);

        if config.track_iterations {
            self.total_iterations
                .fetch_add(iterations, Ordering::Relaxed);
            self.iterations_sum.fetch_add(iterations, Ordering::Relaxed);
        }

        if config.track_paths {
            self.nodes_visited.fetch_add(nodes, Ordering::Relaxed);
        }

        self.total_latency_ms
            .fetch_add(latency_ms, Ordering::Relaxed);
    }

    /// Record a found path.
    pub fn record_path(&self, length: u64, score: f64, config: &RetrievalMetricsConfig) {
        if !config.track_paths {
            return;
        }

        self.paths_found.fetch_add(1, Ordering::Relaxed);
        self.path_length_sum.fetch_add(length, Ordering::Relaxed);

        if config.track_scores {
            let scaled_score = (score * 1_000_000.0) as u64;
            self.path_score_sum_scaled
                .fetch_add(scaled_score, Ordering::Relaxed);

            if score >= 0.5 {
                self.high_score_paths.fetch_add(1, Ordering::Relaxed);
            } else if score < 0.3 {
                self.low_score_paths.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Record a cache hit.
    pub fn record_cache_hit(&self, config: &RetrievalMetricsConfig) {
        if config.track_cache {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record a cache miss.
    pub fn record_cache_miss(&self, config: &RetrievalMetricsConfig) {
        if config.track_cache {
            self.cache_misses.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record a backtrack.
    pub fn record_backtrack(&self) {
        self.backtracks.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a sufficiency check.
    pub fn record_sufficiency_check(&self, was_sufficient: bool) {
        self.sufficiency_checks.fetch_add(1, Ordering::Relaxed);
        if was_sufficient {
            self.sufficient_results.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Reset all metrics.
    pub fn reset(&self) {
        self.total_queries.store(0, Ordering::Relaxed);
        self.total_iterations.store(0, Ordering::Relaxed);
        self.iterations_sum.store(0, Ordering::Relaxed);
        self.nodes_visited.store(0, Ordering::Relaxed);
        self.paths_found.store(0, Ordering::Relaxed);
        self.path_length_sum.store(0, Ordering::Relaxed);
        self.path_score_sum_scaled.store(0, Ordering::Relaxed);
        self.high_score_paths.store(0, Ordering::Relaxed);
        self.low_score_paths.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.total_latency_ms.store(0, Ordering::Relaxed);
        self.backtracks.store(0, Ordering::Relaxed);
        self.sufficiency_checks.store(0, Ordering::Relaxed);
        self.sufficient_results.store(0, Ordering::Relaxed);
    }

    /// Generate a report snapshot.
    pub fn generate_report(&self) -> RetrievalMetricsReport {
        let total_queries = self.total_queries.load(Ordering::Relaxed);
        let paths_found = self.paths_found.load(Ordering::Relaxed);
        let cache_hits = self.cache_hits.load(Ordering::Relaxed);
        let cache_misses = self.cache_misses.load(Ordering::Relaxed);
        let total_cache = cache_hits + cache_misses;
        let sufficiency_checks = self.sufficiency_checks.load(Ordering::Relaxed);

        RetrievalMetricsReport {
            total_queries,
            total_iterations: self.total_iterations.load(Ordering::Relaxed),
            avg_iterations: if total_queries > 0 {
                self.iterations_sum.load(Ordering::Relaxed) as f64 / total_queries as f64
            } else {
                0.0
            },
            nodes_visited: self.nodes_visited.load(Ordering::Relaxed),
            paths_found,
            avg_path_length: if paths_found > 0 {
                self.path_length_sum.load(Ordering::Relaxed) as f64 / paths_found as f64
            } else {
                0.0
            },
            avg_path_score: if paths_found > 0 {
                (self.path_score_sum_scaled.load(Ordering::Relaxed) as f64 / 1_000_000.0)
                    / paths_found as f64
            } else {
                0.0
            },
            high_score_paths: self.high_score_paths.load(Ordering::Relaxed),
            low_score_paths: self.low_score_paths.load(Ordering::Relaxed),
            cache_hits,
            cache_misses,
            cache_hit_rate: if total_cache > 0 {
                cache_hits as f64 / total_cache as f64
            } else {
                0.0
            },
            total_latency_ms: self.total_latency_ms.load(Ordering::Relaxed),
            avg_latency_ms: if total_queries > 0 {
                self.total_latency_ms.load(Ordering::Relaxed) as f64 / total_queries as f64
            } else {
                0.0
            },
            backtracks: self.backtracks.load(Ordering::Relaxed),
            sufficiency_checks,
            sufficiency_rate: if sufficiency_checks > 0 {
                self.sufficient_results.load(Ordering::Relaxed) as f64 / sufficiency_checks as f64
            } else {
                0.0
            },
        }
    }
}

/// Retrieval metrics report.
#[derive(Debug, Clone)]
pub struct RetrievalMetricsReport {
    /// Total number of queries.
    pub total_queries: u64,
    /// Total number of iterations.
    pub total_iterations: u64,
    /// Average iterations per query.
    pub avg_iterations: f64,
    /// Total nodes visited.
    pub nodes_visited: u64,
    /// Total paths found.
    pub paths_found: u64,
    /// Average path length.
    pub avg_path_length: f64,
    /// Average path score.
    pub avg_path_score: f64,
    /// Number of high-score paths (>= 0.5).
    pub high_score_paths: u64,
    /// Number of low-score paths (< 0.3).
    pub low_score_paths: u64,
    /// Number of cache hits.
    pub cache_hits: u64,
    /// Number of cache misses.
    pub cache_misses: u64,
    /// Cache hit rate.
    pub cache_hit_rate: f64,
    /// Total latency in milliseconds.
    pub total_latency_ms: u64,
    /// Average latency per query in milliseconds.
    pub avg_latency_ms: f64,
    /// Number of backtracks.
    pub backtracks: u64,
    /// Number of sufficiency checks.
    pub sufficiency_checks: u64,
    /// Sufficiency rate.
    pub sufficiency_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrieval_metrics_recording() {
        let config = RetrievalMetricsConfig::default();
        let metrics = RetrievalMetrics::new();

        metrics.record_query(5, 10, 100, &config);
        metrics.record_query(3, 8, 80, &config);

        metrics.record_path(3, 0.8, &config);
        metrics.record_path(2, 0.2, &config);

        metrics.record_cache_hit(&config);
        metrics.record_cache_hit(&config);
        metrics.record_cache_miss(&config);

        let report = metrics.generate_report();
        assert_eq!(report.total_queries, 2);
        assert_eq!(report.total_iterations, 8);
        assert_eq!(report.paths_found, 2);
        assert!((report.cache_hit_rate - 0.666).abs() < 0.01);
    }
}
