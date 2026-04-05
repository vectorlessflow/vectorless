// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration types for content aggregation.

use serde::{Deserialize, Serialize};

/// Configuration for content aggregation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAggregatorConfig {
    /// Maximum tokens to return in aggregated content.
    pub token_budget: usize,

    /// Minimum relevance score threshold (0.0 - 1.0).
    /// Content below this threshold will be filtered out.
    pub min_relevance_score: f32,

    /// Scoring strategy for relevance computation.
    pub scoring_strategy: ScoringStrategyConfig,

    /// Output format for aggregated content.
    pub output_format: OutputFormatConfig,

    /// Include relevance scores in output metadata.
    pub include_scores: bool,

    /// Minimum budget allocation per depth level (for hierarchical strategy).
    /// Value between 0.0 and 1.0, representing fraction of total budget.
    pub hierarchical_min_per_level: f32,

    /// Enable content deduplication.
    pub deduplicate: bool,

    /// Similarity threshold for deduplication (0.0 - 1.0).
    pub dedup_threshold: f32,
}

impl Default for ContentAggregatorConfig {
    fn default() -> Self {
        Self {
            token_budget: 4000,
            min_relevance_score: 0.2,
            scoring_strategy: ScoringStrategyConfig::KeywordWithBM25,
            output_format: OutputFormatConfig::Markdown,
            include_scores: false,
            hierarchical_min_per_level: 0.1,
            deduplicate: true,
            dedup_threshold: 0.9,
        }
    }
}

impl ContentAggregatorConfig {
    /// Create a new config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the token budget.
    #[must_use]
    pub fn with_token_budget(mut self, budget: usize) -> Self {
        self.token_budget = budget;
        self
    }

    /// Set the minimum relevance score.
    #[must_use]
    pub fn with_min_relevance(mut self, score: f32) -> Self {
        self.min_relevance_score = score.clamp(0.0, 1.0);
        self
    }

    /// Set the scoring strategy.
    #[must_use]
    pub fn with_scoring_strategy(mut self, strategy: ScoringStrategyConfig) -> Self {
        self.scoring_strategy = strategy;
        self
    }

    /// Set the output format.
    #[must_use]
    pub fn with_output_format(mut self, format: OutputFormatConfig) -> Self {
        self.output_format = format;
        self
    }
}

/// Scoring strategy configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoringStrategyConfig {
    /// Fast keyword matching only.
    KeywordOnly,
    /// Keyword matching with BM25 scoring.
    KeywordWithBM25,
    /// Hybrid: keyword + LLM reranking for top candidates.
    Hybrid,
}

impl Default for ScoringStrategyConfig {
    fn default() -> Self {
        Self::KeywordWithBM25
    }
}

/// Output format configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormatConfig {
    /// Markdown format with headers.
    Markdown,
    /// JSON format.
    Json,
    /// Tree format.
    Tree,
    /// Flat text format.
    Flat,
}

impl Default for OutputFormatConfig {
    fn default() -> Self {
        Self::Markdown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ContentAggregatorConfig::default();
        assert_eq!(config.token_budget, 4000);
        assert_eq!(config.min_relevance_score, 0.2);
    }

    #[test]
    fn test_config_builder() {
        let config = ContentAggregatorConfig::new()
            .with_token_budget(2000)
            .with_min_relevance(0.5);

        assert_eq!(config.token_budget, 2000);
        assert_eq!(config.min_relevance_score, 0.5);
    }

    #[test]
    fn test_min_relevance_clamped() {
        let config = ContentAggregatorConfig::new().with_min_relevance(1.5);
        assert_eq!(config.min_relevance_score, 1.0);

        let config = ContentAggregatorConfig::new().with_min_relevance(-0.5);
        assert_eq!(config.min_relevance_score, 0.0);
    }
}
