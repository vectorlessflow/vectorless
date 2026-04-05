// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Metrics configuration for unified observability.

use serde::{Deserialize, Serialize};

/// Unified metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Storage path for persisted metrics.
    #[serde(default = "default_storage_path")]
    pub storage_path: String,

    /// Retention period in days.
    #[serde(default = "default_retention_days")]
    pub retention_days: usize,

    /// LLM metrics configuration.
    #[serde(default)]
    pub llm: LlmMetricsConfig,

    /// Pilot metrics configuration.
    #[serde(default)]
    pub pilot: PilotMetricsConfig,

    /// Retrieval metrics configuration.
    #[serde(default)]
    pub retrieval: RetrievalMetricsConfig,
}

fn default_storage_path() -> String {
    "./workspace/metrics".to_string()
}

fn default_retention_days() -> usize {
    30
}

fn default_true() -> bool {
    true
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            storage_path: default_storage_path(),
            retention_days: default_retention_days(),
            llm: LlmMetricsConfig::default(),
            pilot: PilotMetricsConfig::default(),
            retrieval: RetrievalMetricsConfig::default(),
        }
    }
}

impl MetricsConfig {
    /// Create a new metrics config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable metrics collection.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }
}

/// LLM-specific metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMetricsConfig {
    /// Track token usage.
    #[serde(default = "default_true")]
    pub track_tokens: bool,

    /// Track latency.
    #[serde(default = "default_true")]
    pub track_latency: bool,

    /// Track estimated cost.
    #[serde(default = "default_true")]
    pub track_cost: bool,

    /// Cost per 1K input tokens (in USD).
    #[serde(default = "default_cost_per_1k_input")]
    pub cost_per_1k_input_tokens: f64,

    /// Cost per 1K output tokens (in USD).
    #[serde(default = "default_cost_per_1k_output")]
    pub cost_per_1k_output_tokens: f64,
}

fn default_cost_per_1k_input() -> f64 {
    0.00015 // gpt-4o-mini
}

fn default_cost_per_1k_output() -> f64 {
    0.0006 // gpt-4o-mini
}

impl Default for LlmMetricsConfig {
    fn default() -> Self {
        Self {
            track_tokens: default_true(),
            track_latency: default_true(),
            track_cost: default_true(),
            cost_per_1k_input_tokens: default_cost_per_1k_input(),
            cost_per_1k_output_tokens: default_cost_per_1k_output(),
        }
    }
}

impl LlmMetricsConfig {
    /// Calculate cost for given tokens.
    pub fn calculate_cost(&self, input_tokens: u64, output_tokens: u64) -> f64 {
        (input_tokens as f64 / 1000.0) * self.cost_per_1k_input_tokens
            + (output_tokens as f64 / 1000.0) * self.cost_per_1k_output_tokens
    }
}

/// Pilot-specific metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PilotMetricsConfig {
    /// Track Pilot decisions.
    #[serde(default = "default_true")]
    pub track_decisions: bool,

    /// Track decision accuracy (requires feedback).
    #[serde(default = "default_true")]
    pub track_accuracy: bool,

    /// Track user feedback.
    #[serde(default = "default_true")]
    pub track_feedback: bool,
}

impl Default for PilotMetricsConfig {
    fn default() -> Self {
        Self {
            track_decisions: default_true(),
            track_accuracy: default_true(),
            track_feedback: default_true(),
        }
    }
}

/// Retrieval-specific metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalMetricsConfig {
    /// Track search paths.
    #[serde(default = "default_true")]
    pub track_paths: bool,

    /// Track relevance scores.
    #[serde(default = "default_true")]
    pub track_scores: bool,

    /// Track iterations.
    #[serde(default = "default_true")]
    pub track_iterations: bool,

    /// Track cache hits/misses.
    #[serde(default = "default_true")]
    pub track_cache: bool,
}

impl Default for RetrievalMetricsConfig {
    fn default() -> Self {
        Self {
            track_paths: default_true(),
            track_scores: default_true(),
            track_iterations: default_true(),
            track_cache: default_true(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_config_defaults() {
        let config = MetricsConfig::default();
        assert!(config.enabled);
        assert_eq!(config.retention_days, 30);
    }

    #[test]
    fn test_llm_cost_calculation() {
        let config = LlmMetricsConfig::default();

        // 1000 input + 500 output tokens
        let cost = config.calculate_cost(1000, 500);

        // 1 * 0.00015 + 0.5 * 0.0006 = 0.00015 + 0.0003 = 0.00045
        assert!((cost - 0.00045).abs() < 0.000001);
    }

    #[test]
    fn test_disabled_metrics() {
        let config = MetricsConfig::disabled();
        assert!(!config.enabled);
    }
}
