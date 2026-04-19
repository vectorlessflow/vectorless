// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Runtime LLM configuration types.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Runtime LLM client configuration.
///
/// This is the runtime representation used by [`LlmClient`](super::LlmClient).
/// Created from the config-layer [`LlmConfig`](crate::config::LlmConfig)
/// during pool construction — users never construct this directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Model name (e.g., "gpt-4o-mini", "gpt-4o").
    #[serde(default)]
    pub model: String,

    /// API endpoint URL.
    #[serde(default)]
    pub endpoint: String,

    /// API key.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for response.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Temperature for generation.
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Retry configuration.
    #[serde(default)]
    pub retry: RetryConfig,

    /// Per-request timeout. 0 means no timeout (wait indefinitely).
    #[serde(default)]
    pub request_timeout_secs: u64,
}

fn default_max_tokens() -> usize {
    2000
}

fn default_temperature() -> f32 {
    0.0
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model: String::new(),
            endpoint: String::new(),
            api_key: None,
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            retry: RetryConfig::default(),
            request_timeout_secs: 0,
        }
    }
}

impl LlmConfig {
    /// Create a new config with a specific model.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            ..Self::default()
        }
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

    /// Set the max tokens.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set the temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Set the retry configuration.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }
}

/// Runtime retry configuration for LLM calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (including initial call).
    #[serde(default = "default_max_attempts")]
    pub max_attempts: usize,

    /// Initial delay before first retry (milliseconds).
    #[serde(default = "default_initial_delay_ms")]
    pub initial_delay_ms: u64,

    /// Maximum delay between retries (milliseconds).
    #[serde(default = "default_max_delay_ms")]
    pub max_delay_ms: u64,

    /// Multiplier for exponential backoff.
    #[serde(default = "default_multiplier")]
    pub multiplier: f64,

    /// Whether to retry on rate limit errors.
    #[serde(default = "default_true")]
    pub retry_on_rate_limit: bool,
}

fn default_max_attempts() -> usize {
    3
}
fn default_initial_delay_ms() -> u64 {
    500
}
fn default_max_delay_ms() -> u64 {
    30000
}
fn default_multiplier() -> f64 {
    2.0
}
fn default_true() -> bool {
    true
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            initial_delay_ms: default_initial_delay_ms(),
            max_delay_ms: default_max_delay_ms(),
            multiplier: default_multiplier(),
            retry_on_rate_limit: default_true(),
        }
    }
}

impl RetryConfig {
    /// Create a new retry config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of attempts.
    pub fn with_max_attempts(mut self, max_attempts: usize) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Set the initial delay (milliseconds).
    pub fn with_initial_delay(mut self, delay_ms: u64) -> Self {
        self.initial_delay_ms = delay_ms;
        self
    }

    /// Set the maximum delay (milliseconds).
    pub fn with_max_delay(mut self, delay_ms: u64) -> Self {
        self.max_delay_ms = delay_ms;
        self
    }

    /// Set the backoff multiplier.
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }

    /// Set whether to retry on rate limit.
    pub fn with_retry_on_rate_limit(mut self, retry: bool) -> Self {
        self.retry_on_rate_limit = retry;
        self
    }

    /// Calculate delay for a given attempt (0-indexed).
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        let delay_ms = (self.initial_delay_ms as f64) * self.multiplier.powf(attempt as f64);
        let delay_ms = delay_ms.min(self.max_delay_ms as f64);
        Duration::from_millis(delay_ms as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_delay_calculation() {
        let config = RetryConfig::default();

        // Initial delay is 500ms
        assert_eq!(config.delay_for_attempt(0), Duration::from_millis(500));

        // Second attempt: 500 * 2 = 1000ms
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(1000));

        // Third attempt: 500 * 4 = 2000ms
        assert_eq!(config.delay_for_attempt(2), Duration::from_millis(2000));
    }

    #[test]
    fn test_retry_delay_max_cap() {
        let config = RetryConfig {
            max_delay_ms: 1500,
            ..RetryConfig::default()
        };

        // Should cap at max_delay_ms
        assert_eq!(config.delay_for_attempt(5), Duration::from_millis(1500));
    }

    #[test]
    fn test_llm_config_builder() {
        let config = LlmConfig::new("gpt-4o")
            .with_max_tokens(1000)
            .with_temperature(0.5);

        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.max_tokens, 1000);
        assert!((config.temperature - 0.5).abs() < 0.001);
    }
}
