// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration merging.
//!
//! This module provides utilities for merging multiple configurations,
//! enabling layered configuration from multiple sources.

use super::types::{
    CacheConfig, ConcurrencyConfig, Config, ContentAggregatorConfig, FallbackConfig, IndexerConfig,
    RetrievalConfig, SearchConfig, StorageConfig, StrategyConfig, SufficiencyConfig, SummaryConfig,
};

/// Configuration merge strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Replace with source value.
    Replace,
    /// Keep existing value if present (don't overwrite).
    KeepExisting,
    /// Recursively merge nested structures.
    Recursive,
}

/// Trait for configuration merging.
pub trait Merge {
    /// Merge another configuration into this one.
    fn merge(&mut self, other: &Self, strategy: MergeStrategy);
}

impl Merge for Config {
    fn merge(&mut self, other: &Self, strategy: MergeStrategy) {
        self.indexer.merge(&other.indexer, strategy);
        self.summary.merge(&other.summary, strategy);
        self.retrieval.merge(&other.retrieval, strategy);
        self.storage.merge(&other.storage, strategy);
        self.concurrency.merge(&other.concurrency, strategy);
        self.fallback.merge(&other.fallback, strategy);
    }
}

impl Merge for IndexerConfig {
    fn merge(&mut self, other: &Self, strategy: MergeStrategy) {
        if strategy == MergeStrategy::Replace || self.subsection_threshold == 300 {
            self.subsection_threshold = other.subsection_threshold;
        }
        if strategy == MergeStrategy::Replace || self.max_segment_tokens == 3000 {
            self.max_segment_tokens = other.max_segment_tokens;
        }
        if strategy == MergeStrategy::Replace || self.max_summary_tokens == 200 {
            self.max_summary_tokens = other.max_summary_tokens;
        }
        if strategy == MergeStrategy::Replace || self.min_summary_tokens == 20 {
            self.min_summary_tokens = other.min_summary_tokens;
        }
    }
}

impl Merge for SummaryConfig {
    fn merge(&mut self, other: &Self, strategy: MergeStrategy) {
        if strategy == MergeStrategy::Replace || self.model == "gpt-4o-mini" {
            self.model = other.model.clone();
        }
        if strategy == MergeStrategy::Replace || self.endpoint == "https://api.openai.com/v1" {
            self.endpoint = other.endpoint.clone();
        }
        // Always merge API keys if present
        if other.api_key.is_some() {
            self.api_key = other.api_key.clone();
        }
        if strategy == MergeStrategy::Replace || self.max_tokens == 200 {
            self.max_tokens = other.max_tokens;
        }
        if strategy == MergeStrategy::Replace || self.temperature == 0.0 {
            self.temperature = other.temperature;
        }
    }
}

impl Merge for RetrievalConfig {
    fn merge(&mut self, other: &Self, strategy: MergeStrategy) {
        if strategy == MergeStrategy::Replace || self.model == "gpt-4o" {
            self.model = other.model.clone();
        }
        if strategy == MergeStrategy::Replace || self.endpoint == "https://api.openai.com/v1" {
            self.endpoint = other.endpoint.clone();
        }
        if other.api_key.is_some() {
            self.api_key = other.api_key.clone();
        }
        if strategy == MergeStrategy::Replace || self.max_tokens == 1000 {
            self.max_tokens = other.max_tokens;
        }
        if strategy == MergeStrategy::Replace || self.temperature == 0.0 {
            self.temperature = other.temperature;
        }
        if strategy == MergeStrategy::Replace || self.top_k == 3 {
            self.top_k = other.top_k;
        }

        self.search.merge(&other.search, strategy);
        self.sufficiency.merge(&other.sufficiency, strategy);
        self.cache.merge(&other.cache, strategy);
        self.strategy.merge(&other.strategy, strategy);
        self.content.merge(&other.content, strategy);
    }
}

impl Merge for SearchConfig {
    fn merge(&mut self, other: &Self, strategy: MergeStrategy) {
        if strategy == MergeStrategy::Replace || self.top_k == 5 {
            self.top_k = other.top_k;
        }
        if strategy == MergeStrategy::Replace || self.beam_width == 3 {
            self.beam_width = other.beam_width;
        }
        if strategy == MergeStrategy::Replace || self.max_iterations == 10 {
            self.max_iterations = other.max_iterations;
        }
        if strategy == MergeStrategy::Replace || (self.min_score - 0.1).abs() < f32::EPSILON {
            self.min_score = other.min_score;
        }
    }
}

impl Merge for SufficiencyConfig {
    fn merge(&mut self, other: &Self, strategy: MergeStrategy) {
        if strategy == MergeStrategy::Replace || self.min_tokens == 500 {
            self.min_tokens = other.min_tokens;
        }
        if strategy == MergeStrategy::Replace || self.target_tokens == 2000 {
            self.target_tokens = other.target_tokens;
        }
        if strategy == MergeStrategy::Replace || self.max_tokens == 4000 {
            self.max_tokens = other.max_tokens;
        }
        if strategy == MergeStrategy::Replace || self.min_content_length == 200 {
            self.min_content_length = other.min_content_length;
        }
        if strategy == MergeStrategy::Replace
            || (self.confidence_threshold - 0.7).abs() < f32::EPSILON
        {
            self.confidence_threshold = other.confidence_threshold;
        }
    }
}

impl Merge for CacheConfig {
    fn merge(&mut self, other: &Self, strategy: MergeStrategy) {
        if strategy == MergeStrategy::Replace || self.max_entries == 1000 {
            self.max_entries = other.max_entries;
        }
        if strategy == MergeStrategy::Replace || self.ttl_secs == 3600 {
            self.ttl_secs = other.ttl_secs;
        }
    }
}

impl Merge for StrategyConfig {
    fn merge(&mut self, other: &Self, strategy: MergeStrategy) {
        if strategy == MergeStrategy::Replace || (self.exploration_weight - 1.414).abs() < 0.001 {
            self.exploration_weight = other.exploration_weight;
        }
        if strategy == MergeStrategy::Replace
            || (self.similarity_threshold - 0.5).abs() < f32::EPSILON
        {
            self.similarity_threshold = other.similarity_threshold;
        }
        if strategy == MergeStrategy::Replace
            || (self.high_similarity_threshold - 0.8).abs() < f32::EPSILON
        {
            self.high_similarity_threshold = other.high_similarity_threshold;
        }
        if strategy == MergeStrategy::Replace
            || (self.low_similarity_threshold - 0.3).abs() < f32::EPSILON
        {
            self.low_similarity_threshold = other.low_similarity_threshold;
        }
    }
}

impl Merge for ContentAggregatorConfig {
    fn merge(&mut self, other: &Self, strategy: MergeStrategy) {
        if other.enabled != self.enabled {
            self.enabled = other.enabled;
        }
        if strategy == MergeStrategy::Replace || self.token_budget == 4000 {
            self.token_budget = other.token_budget;
        }
        if strategy == MergeStrategy::Replace
            || (self.min_relevance_score - 0.2).abs() < f32::EPSILON
        {
            self.min_relevance_score = other.min_relevance_score;
        }
        if strategy == MergeStrategy::Replace || self.scoring_strategy == "keyword_bm25" {
            self.scoring_strategy = other.scoring_strategy.clone();
        }
        if strategy == MergeStrategy::Replace || self.output_format == "markdown" {
            self.output_format = other.output_format.clone();
        }
        if other.include_scores != self.include_scores {
            self.include_scores = other.include_scores;
        }
        if strategy == MergeStrategy::Replace
            || (self.hierarchical_min_per_level - 0.1).abs() < f32::EPSILON
        {
            self.hierarchical_min_per_level = other.hierarchical_min_per_level;
        }
        if other.deduplicate != self.deduplicate {
            self.deduplicate = other.deduplicate;
        }
        if strategy == MergeStrategy::Replace || (self.dedup_threshold - 0.9).abs() < f32::EPSILON {
            self.dedup_threshold = other.dedup_threshold;
        }
    }
}

impl Merge for StorageConfig {
    fn merge(&mut self, other: &Self, strategy: MergeStrategy) {
        if strategy == MergeStrategy::Replace {
            self.workspace_dir = other.workspace_dir.clone();
        }
    }
}

impl Merge for ConcurrencyConfig {
    fn merge(&mut self, other: &Self, strategy: MergeStrategy) {
        if strategy == MergeStrategy::Replace || self.max_concurrent_requests == 10 {
            self.max_concurrent_requests = other.max_concurrent_requests;
        }
        if strategy == MergeStrategy::Replace || self.requests_per_minute == 500 {
            self.requests_per_minute = other.requests_per_minute;
        }
        if other.enabled != self.enabled {
            self.enabled = other.enabled;
        }
        if other.semaphore_enabled != self.semaphore_enabled {
            self.semaphore_enabled = other.semaphore_enabled;
        }
    }
}

impl Merge for FallbackConfig {
    fn merge(&mut self, other: &Self, strategy: MergeStrategy) {
        if other.enabled != self.enabled {
            self.enabled = other.enabled;
        }
        if !other.models.is_empty() {
            self.models = other.models.clone();
        }
        if !other.endpoints.is_empty() {
            self.endpoints = other.endpoints.clone();
        }
        if strategy == MergeStrategy::Replace {
            self.on_rate_limit = other.on_rate_limit;
            self.on_timeout = other.on_timeout;
            self.on_all_failed = other.on_all_failed;
            self.max_retries = other.max_retries;
            self.initial_retry_delay_ms = other.initial_retry_delay_ms;
            self.max_retry_delay_ms = other.max_retry_delay_ms;
            self.retry_multiplier = other.retry_multiplier;
        }
    }
}

/// Configuration overlay for layered configuration.
///
/// Allows building a configuration from multiple sources,
/// with later overlays taking precedence.
#[derive(Debug, Clone)]
pub struct ConfigOverlay {
    /// Base configuration.
    base: Config,
    /// Overlay configurations (applied in order).
    overlays: Vec<Config>,
}

impl ConfigOverlay {
    /// Create a new overlay with a base configuration.
    pub fn new(base: Config) -> Self {
        Self {
            base,
            overlays: Vec::new(),
        }
    }

    /// Add an overlay configuration.
    pub fn overlay(mut self, config: Config) -> Self {
        self.overlays.push(config);
        self
    }

    /// Resolve all overlays into a final configuration.
    pub fn resolve(self) -> Config {
        let mut result = self.base;
        for overlay in self.overlays {
            result.merge(&overlay, MergeStrategy::Replace);
        }
        result
    }
}

impl Default for ConfigOverlay {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_merge() {
        let mut base = Config::default();
        let mut overlay = Config::default();

        overlay.retrieval.top_k = 10;
        overlay.summary.model = "gpt-4o".to_string();

        base.merge(&overlay, MergeStrategy::Replace);

        assert_eq!(base.retrieval.top_k, 10);
        assert_eq!(base.summary.model, "gpt-4o");
    }

    #[test]
    fn test_config_overlay() {
        let mut overlay1 = Config::default();
        overlay1.retrieval.top_k = 5;

        let mut overlay2 = Config::default();
        overlay2.retrieval.top_k = 10;

        let config = ConfigOverlay::new(Config::default())
            .overlay(overlay1)
            .overlay(overlay2)
            .resolve();

        assert_eq!(config.retrieval.top_k, 10);
    }

    #[test]
    fn test_merge_keeps_api_keys() {
        let mut base = Config::default();
        let mut overlay = Config::default();

        overlay.summary.api_key = Some("test-key".to_string());

        base.merge(&overlay, MergeStrategy::Replace);

        assert_eq!(base.summary.api_key, Some("test-key".to_string()));
    }
}
