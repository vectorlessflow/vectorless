// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Result merging and deduplication.
//!
//! This module provides strategies for merging and deduplicating
//! retrieval results from multiple sources.

use std::collections::{HashMap, HashSet};

use super::scorer::ScoredResult;
use crate::core::retriever::RetrievalResult;

/// Merge strategy for combining results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Keep all results, sort by score.
    Concat,

    /// Deduplicate by title, keep highest score.
    DeduplicateTitle,

    /// Deduplicate by node ID, keep highest score.
    DeduplicateNodeId,

    /// Reciprocal rank fusion.
    ReciprocalRankFusion,
}

impl Default for MergeStrategy {
    fn default() -> Self {
        Self::DeduplicateTitle
    }
}

/// Merger for combining and deduplicating results.
#[derive(Debug, Clone)]
pub struct Merger {
    /// The merge strategy to use.
    strategy: MergeStrategy,

    /// Minimum score threshold for filtering.
    min_score: f32,

    /// Maximum number of results to return.
    max_results: usize,

    /// RRF k parameter for reciprocal rank fusion.
    rrf_k: f32,
}

impl Merger {
    /// Create a new merger with default settings.
    pub fn new() -> Self {
        Self {
            strategy: MergeStrategy::default(),
            min_score: 0.0,
            max_results: 10,
            rrf_k: 60.0,
        }
    }

    /// Set the merge strategy.
    pub fn with_strategy(mut self, strategy: MergeStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set the minimum score threshold.
    pub fn with_min_score(mut self, score: f32) -> Self {
        self.min_score = score;
        self
    }

    /// Set the maximum number of results.
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }

    /// Set the RRF k parameter.
    pub fn with_rrf_k(mut self, k: f32) -> Self {
        self.rrf_k = k;
        self
    }

    /// Merge a list of scored results.
    pub fn merge(&self, results: Vec<ScoredResult>) -> Vec<ScoredResult> {
        let merged = match self.strategy {
            MergeStrategy::Concat => {
                self.merge_concat(results)
            }
            MergeStrategy::DeduplicateTitle => {
                self.merge_dedup_title(results)
            }
            MergeStrategy::DeduplicateNodeId => {
                self.merge_dedup_node_id(results)
            }
            MergeStrategy::ReciprocalRankFusion => {
                self.merge_rrf(results)
            }
        };

        // Apply min score filter
        let filtered: Vec<ScoredResult> = merged
            .into_iter()
            .filter(|r| r.score >= self.min_score)
            .collect();

        // Truncate to max results
        let mut final_results = filtered;
        final_results.truncate(self.max_results);
        final_results
    }

    /// Merge results from multiple retrievers.
    pub fn merge_multiple(&self, result_sets: Vec<Vec<ScoredResult>>) -> Vec<ScoredResult> {
        let all_results: Vec<ScoredResult> = result_sets.into_iter().flatten().collect();
        self.merge(all_results)
    }

    fn merge_concat(&self, mut results: Vec<ScoredResult>) -> Vec<ScoredResult> {
        results.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    fn merge_dedup_title(&self, mut results: Vec<ScoredResult>) -> Vec<ScoredResult> {
        let mut seen_titles: HashSet<String> = HashSet::new();
        let mut deduped: Vec<ScoredResult> = Vec::new();

        // Sort by score first
        results.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });

        for result in results {
            let title_key = result.result.title.to_lowercase();
            if !seen_titles.contains(&title_key) {
                seen_titles.insert(title_key);
                deduped.push(result);
            }
        }

        deduped
    }

    fn merge_dedup_node_id(&self, mut results: Vec<ScoredResult>) -> Vec<ScoredResult> {
        let mut seen_ids: HashSet<String> = HashSet::new();
        let mut deduped: Vec<ScoredResult> = Vec::new();

        // Sort by score first
        results.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });

        for result in results {
            if let Some(ref node_id) = result.result.node_id {
                if !seen_ids.contains(node_id) {
                    seen_ids.insert(node_id.clone());
                    deduped.push(result);
                }
            } else {
                // No node ID, include it
                deduped.push(result);
            }
        }

        deduped
    }

    fn merge_rrf(&self, results: Vec<ScoredResult>) -> Vec<ScoredResult> {
        // Sort by original score to get ranks
        let mut sorted = results;
        sorted.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Calculate RRF scores
        let mut rrf_scores: HashMap<String, (ScoredResult, f32)> = HashMap::new();

        for (rank, scored) in sorted.into_iter().enumerate() {
            let key = scored.result.node_id.clone()
                .unwrap_or_else(|| scored.result.title.clone());

            let rrf_contribution = 1.0 / (self.rrf_k + (rank + 1) as f32);

            rrf_scores.entry(key)
                .and_modify(|(_, score)| *score += rrf_contribution)
                .or_insert((scored, rrf_contribution));
        }

        // Convert back to vector and sort by RRF score
        let mut final_results: Vec<ScoredResult> = rrf_scores
            .into_iter()
            .map(|(_, (mut result, rrf_score))| {
                result.score = rrf_score;
                result.breakdown.push(("rrf".to_string(), rrf_score));
                result
            })
            .collect();

        final_results.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });

        final_results
    }

    /// Extract just the retrieval results from scored results.
    pub fn extract_results(scored: Vec<ScoredResult>) -> Vec<RetrievalResult> {
        scored.into_iter().map(|s| s.result).collect()
    }

    /// Get the top N results.
    pub fn top_n(&self, results: Vec<ScoredResult>, n: usize) -> Vec<ScoredResult> {
        let mut sorted = results;
        sorted.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(n);
        sorted
    }
}

impl Default for Merger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merger_creation() {
        let merger = Merger::new();
        assert_eq!(merger.strategy, MergeStrategy::DeduplicateTitle);
        assert_eq!(merger.max_results, 10);
    }

    #[test]
    fn test_merge_dedup_title() {
        let merger = Merger::new()
            .with_strategy(MergeStrategy::DeduplicateTitle);

        let results = vec![
            ScoredResult::new(RetrievalResult::new("Test").with_score(0.9), 0.9),
            ScoredResult::new(RetrievalResult::new("Test").with_score(0.5), 0.5),
            ScoredResult::new(RetrievalResult::new("Other").with_score(0.7), 0.7),
        ];

        let merged = merger.merge(results);
        assert_eq!(merged.len(), 2);
        // First "Test" should have higher score
        assert!((merged[0].result.score - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_merge_max_results() {
        let merger = Merger::new()
            .with_max_results(2);

        let results: Vec<ScoredResult> = (0..5)
            .map(|i| ScoredResult::new(
                RetrievalResult::new(format!("Result {}", i)).with_score(1.0 - i as f32 * 0.1),
                1.0 - i as f32 * 0.1
            ))
            .collect();

        let merged = merger.merge(results);
        assert_eq!(merged.len(), 2);
    }
}
