// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Content aggregator configuration types.

use serde::{Deserialize, Serialize};

/// Content aggregator configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAggregatorConfig {
    /// Whether content aggregator is enabled.
    /// When disabled, uses simple content collection (legacy behavior).
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Maximum tokens for aggregated content.
    #[serde(default = "default_token_budget")]
    pub token_budget: usize,

    /// Minimum relevance score threshold (0.0 - 1.0).
    /// Content below this threshold will be filtered out.
    #[serde(default = "default_min_relevance_score")]
    pub min_relevance_score: f32,

    /// Scoring strategy: "keyword_only" | "keyword_bm25" | "hybrid"
    #[serde(default = "default_scoring_strategy")]
    pub scoring_strategy: String,

    /// Output format: "markdown" | "json" | "tree" | "flat"
    #[serde(default = "default_output_format")]
    pub output_format: String,

    /// Include relevance scores in output.
    #[serde(default)]
    pub include_scores: bool,

    /// Minimum budget allocation per depth level (0.0 - 1.0).
    /// Ensures each tree level gets representation.
    #[serde(default = "default_hierarchical_min_per_level")]
    pub hierarchical_min_per_level: f32,

    /// Enable content deduplication.
    #[serde(default = "default_true")]
    pub deduplicate: bool,

    /// Similarity threshold for deduplication (0.0 - 1.0).
    /// Higher = more aggressive deduplication.
    #[serde(default = "default_dedup_threshold")]
    pub dedup_threshold: f32,
}

fn default_true() -> bool {
    true
}

fn default_token_budget() -> usize {
    4000
}

fn default_min_relevance_score() -> f32 {
    0.2
}

fn default_scoring_strategy() -> String {
    "keyword_bm25".to_string()
}

fn default_output_format() -> String {
    "markdown".to_string()
}

fn default_hierarchical_min_per_level() -> f32 {
    0.1
}

fn default_dedup_threshold() -> f32 {
    0.9
}

impl Default for ContentAggregatorConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            token_budget: default_token_budget(),
            min_relevance_score: default_min_relevance_score(),
            scoring_strategy: default_scoring_strategy(),
            output_format: default_output_format(),
            include_scores: false,
            hierarchical_min_per_level: default_hierarchical_min_per_level(),
            deduplicate: default_true(),
            dedup_threshold: default_dedup_threshold(),
        }
    }
}

impl ContentAggregatorConfig {
    /// Create a new config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable content aggregator (use legacy behavior).
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    /// Set the token budget.
    pub fn with_token_budget(mut self, budget: usize) -> Self {
        self.token_budget = budget;
        self
    }

    /// Set the minimum relevance score.
    pub fn with_min_relevance(mut self, score: f32) -> Self {
        self.min_relevance_score = score.clamp(0.0, 1.0);
        self
    }

    /// Set the scoring strategy.
    pub fn with_scoring_strategy(mut self, strategy: impl Into<String>) -> Self {
        self.scoring_strategy = strategy.into();
        self
    }

    /// Set the output format.
    pub fn with_output_format(mut self, format: impl Into<String>) -> Self {
        self.output_format = format.into();
        self
    }

    /// Enable/disable score inclusion.
    pub fn with_include_scores(mut self, include: bool) -> Self {
        self.include_scores = include;
        self
    }

    /// Enable/disable deduplication.
    pub fn with_deduplicate(mut self, dedupe: bool) -> Self {
        self.deduplicate = dedupe;
        self
    }

    /// Convert to the retrieval content aggregator config.
    pub fn to_aggregator_config(&self) -> crate::retrieval::content::ContentAggregatorConfig {
        use crate::retrieval::content::{
            ContentAggregatorConfig as RetrievalContentConfig, OutputFormatConfig,
            ScoringStrategyConfig,
        };

        let scoring_strategy = match self.scoring_strategy.as_str() {
            "keyword_only" => ScoringStrategyConfig::KeywordOnly,
            "hybrid" => ScoringStrategyConfig::Hybrid,
            _ => ScoringStrategyConfig::KeywordWithBM25,
        };

        let output_format = match self.output_format.as_str() {
            "json" => OutputFormatConfig::Json,
            "tree" => OutputFormatConfig::Tree,
            "flat" => OutputFormatConfig::Flat,
            _ => OutputFormatConfig::Markdown,
        };

        RetrievalContentConfig {
            token_budget: self.token_budget,
            min_relevance_score: self.min_relevance_score,
            scoring_strategy,
            output_format,
            include_scores: self.include_scores,
            hierarchical_min_per_level: self.hierarchical_min_per_level,
            deduplicate: self.deduplicate,
            dedup_threshold: self.dedup_threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_aggregator_config_defaults() {
        let config = ContentAggregatorConfig::default();
        assert!(config.enabled);
        assert_eq!(config.token_budget, 4000);
        assert_eq!(config.min_relevance_score, 0.2);
        assert_eq!(config.scoring_strategy, "keyword_bm25");
        assert_eq!(config.output_format, "markdown");
        assert!(config.deduplicate);
    }

    #[test]
    fn test_content_aggregator_config_disabled() {
        let config = ContentAggregatorConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_content_aggregator_config_builder() {
        let config = ContentAggregatorConfig::new()
            .with_token_budget(8000)
            .with_min_relevance(0.5)
            .with_scoring_strategy("hybrid")
            .with_output_format("json");

        assert_eq!(config.token_budget, 8000);
        assert_eq!(config.min_relevance_score, 0.5);
        assert_eq!(config.scoring_strategy, "hybrid");
        assert_eq!(config.output_format, "json");
    }

    #[test]
    fn test_min_relevance_clamping() {
        let config = ContentAggregatorConfig::new().with_min_relevance(1.5);
        assert_eq!(config.min_relevance_score, 1.0);

        let config = ContentAggregatorConfig::new().with_min_relevance(-0.5);
        assert_eq!(config.min_relevance_score, 0.0);
    }
}
