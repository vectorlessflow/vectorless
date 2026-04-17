// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval strategy configuration types.
//!
//! LLM configuration (model, api_key, endpoint) is managed centrally
//! in [`LlmConfig`](super::LlmConfig). This module only contains
//! retrieval strategy parameters.

use serde::{Deserialize, Serialize};

use super::content::ContentAggregatorConfig;
use super::storage::{CacheConfig, StrategyConfig, SufficiencyConfig};

/// Retrieval strategy configuration.
///
/// Controls how documents are searched and retrieved, independent
/// of which LLM model is used for navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalConfig {
    /// Number of top-k results to return.
    #[serde(default = "default_top_k")]
    pub top_k: usize,

    /// Search algorithm configuration.
    #[serde(default)]
    pub search: SearchConfig,

    /// Sufficiency checker configuration.
    #[serde(default)]
    pub sufficiency: SufficiencyConfig,

    /// Cache configuration.
    #[serde(default)]
    pub cache: CacheConfig,

    /// Strategy-specific configuration.
    #[serde(default)]
    pub strategy: StrategyConfig,

    /// Content aggregator configuration.
    #[serde(default)]
    pub content: ContentAggregatorConfig,
}

fn default_top_k() -> usize {
    3
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            top_k: default_top_k(),
            search: SearchConfig::default(),
            sufficiency: SufficiencyConfig::default(),
            cache: CacheConfig::default(),
            strategy: StrategyConfig::default(),
            content: ContentAggregatorConfig::default(),
        }
    }
}

impl RetrievalConfig {
    /// Create a new retrieval config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the top_k.
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k;
        self
    }
}

/// Search algorithm configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Number of top-k results to return.
    #[serde(default = "default_search_top_k")]
    pub top_k: usize,

    /// Beam width for multi-path search.
    #[serde(default = "default_beam_width")]
    pub beam_width: usize,

    /// Maximum iterations for search algorithms.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,

    /// Minimum score to include a path.
    #[serde(default = "default_min_score")]
    pub min_score: f32,

    /// Fallback chain: algorithms tried in order until min_score is met.
    /// Options: "beam", "mcts", "pure_pilot".
    /// Default: ["beam", "mcts", "pure_pilot"]
    #[serde(default = "default_fallback_chain")]
    pub fallback_chain: Vec<String>,
}

fn default_search_top_k() -> usize {
    5
}

fn default_beam_width() -> usize {
    3
}

fn default_max_iterations() -> usize {
    10
}

fn default_min_score() -> f32 {
    0.1
}
fn default_fallback_chain() -> Vec<String> {
    vec!["beam".into(), "mcts".into(), "pure_pilot".into()]
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            top_k: default_search_top_k(),
            beam_width: default_beam_width(),
            max_iterations: default_max_iterations(),
            min_score: default_min_score(),
            fallback_chain: default_fallback_chain(),
        }
    }
}

impl SearchConfig {
    /// Create new search config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the top_k.
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k;
        self
    }

    /// Set the beam width.
    pub fn with_beam_width(mut self, width: usize) -> Self {
        self.beam_width = width;
        self
    }

    /// Set the max iterations.
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrieval_config_defaults() {
        let config = RetrievalConfig::default();
        assert_eq!(config.top_k, 3);
        assert_eq!(config.search.top_k, 5);
    }

    #[test]
    fn test_search_config_defaults() {
        let config = SearchConfig::default();
        assert_eq!(config.top_k, 5);
        assert_eq!(config.beam_width, 3);
        assert_eq!(config.max_iterations, 10);
    }
}
