// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval configuration types.

use serde::{Deserialize, Serialize};

use super::content::ContentAggregatorConfig;
use super::storage::{CacheConfig, StrategyConfig, SufficiencyConfig};

/// Retrieval model configuration (for navigation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalConfig {
    /// Model name for retrieval/navigation.
    #[serde(default = "default_retrieval_model")]
    pub model: String,

    /// API endpoint for retrieval model.
    #[serde(default = "default_endpoint")]
    pub endpoint: String,

    /// API key.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for retrieval context.
    #[serde(default = "default_max_retrieval_tokens")]
    pub max_tokens: usize,

    /// Temperature for retrieval.
    #[serde(default = "default_temperature")]
    pub temperature: f32,

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

fn default_retrieval_model() -> String {
    "gpt-4o".to_string()
}

fn default_endpoint() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_max_retrieval_tokens() -> usize {
    1000
}

fn default_temperature() -> f32 {
    0.0
}

fn default_top_k() -> usize {
    3
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            model: default_retrieval_model(),
            endpoint: default_endpoint(),
            api_key: None,
            max_tokens: default_max_retrieval_tokens(),
            temperature: default_temperature(),
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

    /// Set the model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the endpoint.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Set the API key.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
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

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            top_k: default_search_top_k(),
            beam_width: default_beam_width(),
            max_iterations: default_max_iterations(),
            min_score: default_min_score(),
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
        assert_eq!(config.model, "gpt-4o");
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
