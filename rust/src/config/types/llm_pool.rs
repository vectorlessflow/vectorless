// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Unified LLM configuration including pool, retry, throttle, and fallback.
//!
//! This module consolidates all LLM-related configuration into a single
//! cohesive structure that maps directly to the TOML configuration file.

use serde::{Deserialize, Serialize};

/// Unified LLM configuration.
///
/// Contains all settings for LLM operations including:
/// - Pool of clients for different purposes (summary, retrieval, pilot)
/// - Retry behavior
/// - Throttle/rate limiting
/// - Fallback strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPoolConfig {
    /// Summary client configuration.
    #[serde(default)]
    pub summary: LlmClientConfig,

    /// Retrieval client configuration.
    #[serde(default)]
    pub retrieval: LlmClientConfig,

    /// Pilot client configuration.
    #[serde(default = "default_pilot_config")]
    pub pilot: LlmClientConfig,

    /// Default API key (used if not specified per-client).
    #[serde(default)]
    pub api_key: Option<String>,

    /// Retry configuration.
    #[serde(default)]
    pub retry: RetryConfig,

    /// Throttle/rate limiting configuration.
    #[serde(default)]
    pub throttle: ThrottleConfig,

    /// Fallback configuration.
    #[serde(default)]
    pub fallback: FallbackConfig,
}

fn default_pilot_config() -> LlmClientConfig {
    LlmClientConfig {
        model: "gpt-4o-mini".to_string(),
        max_tokens: 300,
        temperature: 0.0,
        ..Default::default()
    }
}

impl Default for LlmPoolConfig {
    fn default() -> Self {
        Self {
            summary: LlmClientConfig::default(),
            retrieval: LlmClientConfig {
                model: "gpt-4o".to_string(),
                max_tokens: 100,
                ..Default::default()
            },
            pilot: default_pilot_config(),
            api_key: None,
            retry: RetryConfig::default(),
            throttle: ThrottleConfig::default(),
            fallback: FallbackConfig::default(),
        }
    }
}

impl LlmPoolConfig {
    /// Create a new LLM pool config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default API key.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Get API key for a specific client (client-specific or default).
    pub fn get_api_key_for(&self, client_key: Option<&str>) -> Option<String> {
        // First check client-specific key
        if let Some(key) = client_key {
            if let Some(ref k) = self.summary.api_key {
                if self.summary.model == key {
                    return Some(k.clone());
                }
            }
            if let Some(ref k) = self.retrieval.api_key {
                if self.retrieval.model == key {
                    return Some(k.clone());
                }
            }
            if let Some(ref k) = self.pilot.api_key {
                if self.pilot.model == key {
                    return Some(k.clone());
                }
            }
        }
        // Fall back to default
        self.api_key.clone()
    }
}

/// Individual LLM client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmClientConfig {
    /// Model name.
    #[serde(default = "default_model")]
    pub model: String,

    /// API endpoint.
    #[serde(default = "default_endpoint")]
    pub endpoint: String,

    /// API key (optional, falls back to default).
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for responses.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Temperature for generation.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_endpoint() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_max_tokens() -> usize {
    200
}

fn default_temperature() -> f32 {
    0.0
}

impl Default for LlmClientConfig {
    fn default() -> Self {
        Self {
            model: default_model(),
            endpoint: default_endpoint(),
            api_key: None,
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
        }
    }
}

impl LlmClientConfig {
    /// Create a new client config with defaults.
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

    /// Set the max tokens.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }
}

/// Retry configuration for LLM calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
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

    /// Set the max attempts.
    pub fn with_max_attempts(mut self, max_attempts: usize) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Calculate delay for a given attempt (0-indexed).
    pub fn delay_for_attempt(&self, attempt: usize) -> std::time::Duration {
        let delay_ms =
            (self.initial_delay_ms as f64) * self.multiplier.powi(attempt as i32);
        let delay_ms = delay_ms.min(self.max_delay_ms as f64);
        std::time::Duration::from_millis(delay_ms as u64)
    }
}

/// Throttle/rate limiting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleConfig {
    /// Maximum concurrent LLM API calls.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_requests: usize,

    /// Rate limit: requests per minute.
    #[serde(default = "default_rpm")]
    pub requests_per_minute: usize,

    /// Enable rate limiting.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Enable semaphore-based concurrency limiting.
    #[serde(default = "default_true")]
    pub semaphore_enabled: bool,
}

fn default_max_concurrent() -> usize {
    10
}

fn default_rpm() -> usize {
    500
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: default_max_concurrent(),
            requests_per_minute: default_rpm(),
            enabled: default_true(),
            semaphore_enabled: default_true(),
        }
    }
}

impl ThrottleConfig {
    /// Create a new throttle config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the max concurrent requests.
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent_requests = max;
        self
    }

    /// Set the requests per minute.
    pub fn with_rpm(mut self, rpm: usize) -> Self {
        self.requests_per_minute = rpm;
        self
    }
}

/// Fallback configuration for LLM calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    /// Enable fallback mechanism.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Fallback models in priority order.
    #[serde(default = "default_fallback_models")]
    pub models: Vec<String>,

    /// Fallback endpoints (optional).
    #[serde(default)]
    pub endpoints: Vec<String>,

    /// Behavior on rate limit error.
    #[serde(default)]
    pub on_rate_limit: FallbackBehavior,

    /// Behavior on timeout error.
    #[serde(default)]
    pub on_timeout: FallbackBehavior,

    /// Behavior when all attempts fail.
    #[serde(default)]
    pub on_all_failed: OnAllFailedBehavior,
}

fn default_fallback_models() -> Vec<String> {
    vec!["gpt-4o-mini".to_string(), "glm-4-flash".to_string()]
}

/// Fallback behavior on errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FallbackBehavior {
    /// Retry the same model.
    Retry,
    /// Immediately fall back to next model.
    Fallback,
    /// Retry first, then fall back.
    #[default]
    RetryThenFallback,
    /// Fail immediately.
    Fail,
}

/// Behavior when all fallback attempts fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnAllFailedBehavior {
    /// Return an error.
    #[default]
    ReturnError,
    /// Return cached result if available.
    ReturnCache,
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
        }
    }
}

impl FallbackConfig {
    /// Create a new fallback config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable fallback.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_pool_config_defaults() {
        let config = LlmPoolConfig::default();
        assert_eq!(config.summary.model, "gpt-4o-mini");
        assert_eq!(config.retrieval.model, "gpt-4o");
        assert_eq!(config.pilot.model, "gpt-4o-mini");
        assert_eq!(config.retry.max_attempts, 3);
        assert_eq!(config.throttle.max_concurrent_requests, 10);
    }

    #[test]
    fn test_retry_delay_calculation() {
        let config = RetryConfig::default();

        // Initial delay
        assert_eq!(
            config.delay_for_attempt(0),
            std::time::Duration::from_millis(500)
        );

        // Second attempt: 500 * 2 = 1000
        assert_eq!(
            config.delay_for_attempt(1),
            std::time::Duration::from_millis(1000)
        );
    }

    #[test]
    fn test_fallback_config_defaults() {
        let config = FallbackConfig::default();
        assert!(config.enabled);
        assert!(!config.models.is_empty());
    }
}
