// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Indexer configuration types.

use serde::{Deserialize, Serialize};

/// Indexer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerConfig {
    /// Word count threshold for splitting sections into subsections.
    #[serde(default = "default_subsection_threshold")]
    pub subsection_threshold: usize,

    /// Maximum tokens to send in a single segmentation request.
    #[serde(default = "default_max_segment_tokens")]
    pub max_segment_tokens: usize,

    /// Maximum tokens for each summary.
    #[serde(default = "default_max_summary_tokens")]
    pub max_summary_tokens: usize,

    /// Minimum content tokens required to generate a summary.
    #[serde(default = "default_min_summary_tokens")]
    pub min_summary_tokens: usize,
}

fn default_subsection_threshold() -> usize {
    300
}

fn default_max_segment_tokens() -> usize {
    3000
}

fn default_max_summary_tokens() -> usize {
    200
}

fn default_min_summary_tokens() -> usize {
    20
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            subsection_threshold: default_subsection_threshold(),
            max_segment_tokens: default_max_segment_tokens(),
            max_summary_tokens: default_max_summary_tokens(),
            min_summary_tokens: default_min_summary_tokens(),
        }
    }
}

impl IndexerConfig {
    /// Create a new indexer config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the subsection threshold.
    pub fn with_subsection_threshold(mut self, threshold: usize) -> Self {
        self.subsection_threshold = threshold;
        self
    }

    /// Set the maximum segment tokens.
    pub fn with_max_segment_tokens(mut self, tokens: usize) -> Self {
        self.max_segment_tokens = tokens;
        self
    }

    /// Set the maximum summary tokens.
    pub fn with_max_summary_tokens(mut self, tokens: usize) -> Self {
        self.max_summary_tokens = tokens;
        self
    }

    /// Set the minimum summary tokens.
    pub fn with_min_summary_tokens(mut self, tokens: usize) -> Self {
        self.min_summary_tokens = tokens;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexer_config_defaults() {
        let config = IndexerConfig::default();
        assert_eq!(config.subsection_threshold, 300);
        assert_eq!(config.max_segment_tokens, 3000);
        assert_eq!(config.max_summary_tokens, 200);
        assert_eq!(config.min_summary_tokens, 20);
    }

    #[test]
    fn test_indexer_config_builder() {
        let config = IndexerConfig::new()
            .with_subsection_threshold(500)
            .with_max_summary_tokens(300);

        assert_eq!(config.subsection_threshold, 500);
        assert_eq!(config.max_summary_tokens, 300);
    }
}
