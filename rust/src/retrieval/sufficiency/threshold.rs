// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Threshold-based sufficiency checker.
//!
//! Uses simple heuristics like token count and content length.

use super::{SufficiencyChecker, SufficiencyLevel};
use crate::config::SufficiencyConfig;

/// Configuration for threshold-based checking.
#[derive(Debug, Clone)]
pub struct ThresholdConfig {
    /// Minimum tokens for sufficiency.
    pub min_tokens: usize,
    /// Target tokens for full sufficiency.
    pub target_tokens: usize,
    /// Maximum tokens before stopping.
    pub max_tokens: usize,
    /// Minimum content length (characters).
    pub min_content_length: usize,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self::from_config(&SufficiencyConfig::default())
    }
}

impl ThresholdConfig {
    /// Create from application config.
    pub fn from_config(config: &SufficiencyConfig) -> Self {
        Self {
            min_tokens: config.min_tokens,
            target_tokens: config.target_tokens,
            max_tokens: config.max_tokens,
            min_content_length: config.min_content_length,
        }
    }
}

/// Threshold-based sufficiency checker.
///
/// Uses simple token and length thresholds to determine
/// when enough content has been collected.
pub struct ThresholdChecker {
    config: ThresholdConfig,
}

impl ThresholdChecker {
    /// Create a new threshold checker with default config.
    pub fn new() -> Self {
        Self {
            config: ThresholdConfig::default(),
        }
    }

    /// Create a threshold checker with custom config.
    pub fn with_config(config: ThresholdConfig) -> Self {
        Self { config }
    }

    /// Estimate token count from content.
    fn estimate_tokens(&self, content: &str) -> usize {
        // Rough estimate: ~4 characters per token on average
        content.len() / 4
    }

    /// Check content quality indicators.
    fn check_quality(&self, content: &str) -> f32 {
        let mut score = 0.0;

        // Check for sentence endings (periods, question marks, etc.)
        let sentence_endings = content.matches('.').count()
            + content.matches('?').count()
            + content.matches('!').count();
        score += (sentence_endings as f32 * 0.05).min(0.3);

        // Check for paragraph breaks
        let paragraphs = content.matches("\n\n").count();
        score += (paragraphs as f32 * 0.1).min(0.3);

        // Check for structure markers
        if content.contains(':') || content.contains('-') {
            score += 0.1;
        }

        // Penalize very repetitive content
        let words: Vec<&str> = content.split_whitespace().collect();
        if words.len() > 10 {
            let unique_ratio = words.iter().collect::<std::collections::HashSet<_>>().len() as f32
                / words.len() as f32;
            score += unique_ratio * 0.3;
        }

        score.min(1.0)
    }
}

impl Default for ThresholdChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl SufficiencyChecker for ThresholdChecker {
    fn check(&self, _query: &str, content: &str, token_count: usize) -> SufficiencyLevel {
        let estimated_tokens = if token_count == 0 {
            self.estimate_tokens(content)
        } else {
            token_count
        };

        // Check minimum content length
        if content.len() < self.config.min_content_length {
            return SufficiencyLevel::Insufficient;
        }

        // Check maximum tokens - always sufficient if we hit the limit
        if estimated_tokens >= self.config.max_tokens {
            return SufficiencyLevel::Sufficient;
        }

        // Check target tokens
        if estimated_tokens >= self.config.target_tokens {
            let quality = self.check_quality(content);
            if quality > 0.5 {
                return SufficiencyLevel::Sufficient;
            } else {
                return SufficiencyLevel::PartialSufficient;
            }
        }

        // Check minimum tokens
        if estimated_tokens >= self.config.min_tokens {
            let quality = self.check_quality(content);
            if quality > 0.7 {
                return SufficiencyLevel::PartialSufficient;
            }
        }

        SufficiencyLevel::Insufficient
    }

    fn name(&self) -> &'static str {
        "threshold"
    }
}
