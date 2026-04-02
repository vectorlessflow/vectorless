// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Scoring strategies for ranking retrieval results.

use serde::{Deserialize, Serialize};

use crate::core::retriever::RetrievalResult;

/// A retrieval result with an assigned score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredResult {
    /// The original retrieval result.
    pub result: RetrievalResult,

    /// The computed relevance score (0.0 - 1.0).
    pub score: f32,

    /// Scoring breakdown (strategy -> score).
    pub breakdown: Vec<(String, f32)>,
}

impl ScoredResult {
    /// Create a new scored result.
    pub fn new(result: RetrievalResult, score: f32) -> Self {
        Self {
            result,
            score,
            breakdown: Vec::new(),
        }
    }

    /// Add a score breakdown entry.
    pub fn with_breakdown(mut self, strategy: impl Into<String>, score: f32) -> Self {
        self.breakdown.push((strategy.into(), score));
        self
    }
}

/// Scoring strategy enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoringStrategy {
    /// Use the original score from retrieval.
    Original,

    /// Combine multiple scoring signals.
    Combined,

    /// Score based on term frequency.
    TermFrequency,

    /// Score based on semantic similarity.
    Semantic,
}

impl Default for ScoringStrategy {
    fn default() -> Self {
        Self::Original
    }
}

/// Scorer for ranking retrieval results.
#[derive(Debug, Clone)]
pub struct Scorer {
    /// The scoring strategy to use.
    strategy: ScoringStrategy,

    /// Weight for term frequency scoring.
    tf_weight: f32,

    /// Weight for position scoring (earlier = higher).
    position_weight: f32,

    /// Weight for depth scoring (shallower = higher).
    depth_weight: f32,
}

impl Scorer {
    /// Create a new scorer with default settings.
    pub fn new() -> Self {
        Self {
            strategy: ScoringStrategy::default(),
            tf_weight: 0.3,
            position_weight: 0.2,
            depth_weight: 0.1,
        }
    }

    /// Set the scoring strategy.
    pub fn with_strategy(mut self, strategy: ScoringStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set the term frequency weight.
    pub fn with_tf_weight(mut self, weight: f32) -> Self {
        self.tf_weight = weight;
        self
    }

    /// Set the position weight.
    pub fn with_position_weight(mut self, weight: f32) -> Self {
        self.position_weight = weight;
        self
    }

    /// Set the depth weight.
    pub fn with_depth_weight(mut self, weight: f32) -> Self {
        self.depth_weight = weight;
        self
    }

    /// Score a list of retrieval results.
    pub fn score(&self, results: &[RetrievalResult], query: &str) -> Vec<ScoredResult> {
        match self.strategy {
            ScoringStrategy::Original => {
                results.iter()
                    .map(|r| ScoredResult::new(r.clone(), r.score))
                    .collect()
            }
            ScoringStrategy::Combined => {
                self.score_combined(results, query)
            }
            ScoringStrategy::TermFrequency => {
                self.score_term_frequency(results, query)
            }
            ScoringStrategy::Semantic => {
                // For now, fall back to original scoring
                // TODO: Implement semantic scoring with embeddings
                results.iter()
                    .map(|r| ScoredResult::new(r.clone(), r.score))
                    .collect()
            }
        }
    }

    /// Combined scoring using multiple signals.
    fn score_combined(&self, results: &[RetrievalResult], query: &str) -> Vec<ScoredResult> {
        let query_lower = query.to_lowercase();
        let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

        results.iter().enumerate().map(|(idx, result)| {
            let original_score = result.score;

            // Term frequency score
            let tf_score = self.compute_tf_score(&result.title, &result.content, &query_terms);

            // Position score (earlier results ranked higher)
            let position_score = if results.is_empty() {
                1.0
            } else {
                1.0 - (idx as f32 / results.len() as f32) * 0.5
            };

            // Depth score (shallower nodes ranked higher)
            let depth_score = 1.0 / (1.0 + result.depth as f32 * 0.1);

            // Combined score
            let combined = original_score * 0.4
                + tf_score * self.tf_weight
                + position_score * self.position_weight
                + depth_score * self.depth_weight;

            ScoredResult::new(result.clone(), combined)
                .with_breakdown("original", original_score)
                .with_breakdown("term_frequency", tf_score)
                .with_breakdown("position", position_score)
                .with_breakdown("depth", depth_score)
        }).collect()
    }

    /// Term frequency scoring.
    fn score_term_frequency(&self, results: &[RetrievalResult], query: &str) -> Vec<ScoredResult> {
        let query_lower = query.to_lowercase();
        let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

        results.iter().map(|result| {
            let tf_score = self.compute_tf_score(&result.title, &result.content, &query_terms);
            ScoredResult::new(result.clone(), tf_score)
                .with_breakdown("term_frequency", tf_score)
        }).collect()
    }

    /// Compute term frequency score.
    fn compute_tf_score(&self, title: &str, content: &Option<String>, query_terms: &[&str]) -> f32 {
        if query_terms.is_empty() {
            return 1.0;
        }

        let title_lower = title.to_lowercase();
        let content_lower = content.as_ref()
            .map(|c| c.to_lowercase())
            .unwrap_or_default();

        let mut matches = 0;
        for term in query_terms {
            if title_lower.contains(term) {
                matches += 2; // Title matches count more
            }
            if content_lower.contains(term) {
                matches += 1;
            }
        }

        // Normalize to 0-1 range
        let max_possible = query_terms.len() * 3;
        if max_possible == 0 {
            1.0
        } else {
            (matches as f32 / max_possible as f32).min(1.0)
        }
    }

    /// Sort results by score (descending).
    pub fn sort_by_score(&self, mut scored: Vec<ScoredResult>) -> Vec<ScoredResult> {
        scored.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });
        scored
    }

    /// Filter results below a threshold.
    pub fn filter_by_threshold(&self, scored: Vec<ScoredResult>, min_score: f32) -> Vec<ScoredResult> {
        scored.into_iter()
            .filter(|s| s.score >= min_score)
            .collect()
    }
}

impl Default for Scorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scorer_creation() {
        let scorer = Scorer::new();
        assert_eq!(scorer.strategy, ScoringStrategy::Original);
    }

    #[test]
    fn test_score_original() {
        let scorer = Scorer::new();
        let results = vec![
            RetrievalResult::new("Test 1").with_score(0.8),
            RetrievalResult::new("Test 2").with_score(0.6),
        ];

        let scored = scorer.score(&results, "query");
        assert_eq!(scored.len(), 2);
        assert!((scored[0].score - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_score_combined() {
        let scorer = Scorer::new().with_strategy(ScoringStrategy::Combined);
        let results = vec![
            RetrievalResult::new("Test query match").with_score(0.5),
        ];

        let scored = scorer.score(&results, "query");
        assert_eq!(scored.len(), 1);
        assert!(scored[0].score > 0.0);
        assert!(!scored[0].breakdown.is_empty());
    }
}
