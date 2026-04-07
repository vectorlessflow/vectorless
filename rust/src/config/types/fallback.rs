// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Fallback and error recovery configuration types.

use serde::{Deserialize, Serialize};

/// Fallback behavior when encountering errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackBehavior {
    /// Only retry with the same model/endpoint.
    Retry,
    /// Immediately switch to fallback model/endpoint.
    Fallback,
    /// Retry first, then fallback if still failing.
    RetryThenFallback,
    /// Fail immediately without retry or fallback.
    Fail,
}

impl Default for FallbackBehavior {
    fn default() -> Self {
        Self::RetryThenFallback
    }
}

/// Behavior when all fallback attempts fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnAllFailedBehavior {
    /// Return the error to the caller.
    ReturnError,
    /// Try to return cached result if available.
    ReturnCache,
}

impl Default for OnAllFailedBehavior {
    fn default() -> Self {
        Self::ReturnError
    }
}

/// Fallback configuration for error recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    /// Whether fallback is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Fallback models in priority order.
    #[serde(default = "default_fallback_models")]
    pub models: Vec<String>,

    /// Fallback endpoints in priority order.
    #[serde(default)]
    pub endpoints: Vec<String>,

    /// Behavior on rate limit error (429).
    #[serde(default)]
    pub on_rate_limit: FallbackBehavior,

    /// Behavior on timeout error.
    #[serde(default)]
    pub on_timeout: FallbackBehavior,

    /// Behavior when all attempts fail.
    #[serde(default)]
    pub on_all_failed: OnAllFailedBehavior,

    /// Maximum retry attempts.
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,

    /// Initial retry delay in milliseconds.
    #[serde(default = "default_initial_retry_delay_ms")]
    pub initial_retry_delay_ms: u64,

    /// Maximum retry delay in milliseconds.
    #[serde(default = "default_max_retry_delay_ms")]
    pub max_retry_delay_ms: u64,

    /// Retry delay multiplier (exponential backoff).
    #[serde(default = "default_retry_multiplier")]
    pub retry_multiplier: f32,
}

fn default_fallback_models() -> Vec<String> {
    vec!["gpt-4o-mini".to_string(), "glm-4-flash".to_string()]
}

fn default_max_retries() -> usize {
    3
}

fn default_initial_retry_delay_ms() -> u64 {
    1000
}

fn default_max_retry_delay_ms() -> u64 {
    30000
}

fn default_retry_multiplier() -> f32 {
    2.0
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            models: default_fallback_models(),
            endpoints: Vec::new(),
            on_rate_limit: FallbackBehavior::default(),
            on_timeout: FallbackBehavior::default(),
            on_all_failed: OnAllFailedBehavior::default(),
            max_retries: default_max_retries(),
            initial_retry_delay_ms: default_initial_retry_delay_ms(),
            max_retry_delay_ms: default_max_retry_delay_ms(),
            retry_multiplier: default_retry_multiplier(),
        }
    }
}

fn default_true() -> bool {
    true
}

impl FallbackConfig {
    /// Create a new fallback config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable fallback entirely.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    /// Set fallback models.
    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.models = models;
        self
    }

    /// Set fallback endpoints.
    pub fn with_endpoints(mut self, endpoints: Vec<String>) -> Self {
        self.endpoints = endpoints;
        self
    }

    /// Set behavior on rate limit.
    pub fn with_on_rate_limit(mut self, behavior: FallbackBehavior) -> Self {
        self.on_rate_limit = behavior;
        self
    }

    /// Set behavior on timeout.
    pub fn with_on_timeout(mut self, behavior: FallbackBehavior) -> Self {
        self.on_timeout = behavior;
        self
    }

    /// Set behavior when all attempts fail.
    pub fn with_on_all_failed(mut self, behavior: OnAllFailedBehavior) -> Self {
        self.on_all_failed = behavior;
        self
    }

    /// Set maximum retries.
    pub fn with_max_retries(mut self, max: usize) -> Self {
        self.max_retries = max;
        self
    }

    /// Calculate retry delay with exponential backoff.
    pub fn calculate_retry_delay(&self, attempt: usize) -> std::time::Duration {
        let delay_ms = if attempt == 0 {
            self.initial_retry_delay_ms
        } else {
            let delay =
                self.initial_retry_delay_ms as f32 * self.retry_multiplier.powi(attempt as i32);
            delay.min(self.max_retry_delay_ms as f32) as u64
        };
        std::time::Duration::from_millis(delay_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_config_defaults() {
        let config = FallbackConfig::default();
        assert!(config.enabled);
        assert_eq!(config.models.len(), 2);
        assert_eq!(config.on_rate_limit, FallbackBehavior::RetryThenFallback);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_fallback_config_disabled() {
        let config = FallbackConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_fallback_behavior_serde() {
        let behavior = FallbackBehavior::RetryThenFallback;
        let json = serde_json::to_string(&behavior).unwrap();
        assert_eq!(json, "\"retry_then_fallback\"");

        let decoded: FallbackBehavior = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, behavior);
    }

    #[test]
    fn test_retry_delay_calculation() {
        let config = FallbackConfig::default();

        let d0 = config.calculate_retry_delay(0);
        let d1 = config.calculate_retry_delay(1);
        let d2 = config.calculate_retry_delay(2);

        assert_eq!(d0.as_millis(), 1000);
        assert_eq!(d1.as_millis(), 2000);
        assert_eq!(d2.as_millis(), 4000);
    }
}
