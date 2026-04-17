// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Unified LLM configuration.
//!
//! This module consolidates all LLM-related configuration into a single
//! cohesive structure. Users configure via [`EngineBuilder`](crate::client::EngineBuilder)
//! for simple cases, or construct [`LlmConfig`] programmatically for advanced use.

use serde::{Deserialize, Serialize};

/// Unified LLM configuration — the single entry point for all LLM settings.
///
/// Contains:
/// - Global credentials (`api_key`, `model`, `endpoint`)
/// - Per-purpose slot overrides (`index`, `retrieval`, `pilot`)
/// - Infrastructure settings (`retry`, `throttle`, `fallback`)
///
/// # Simple usage (via EngineBuilder)
///
/// ```rust,no_run
/// use vectorless::client::EngineBuilder;
///
/// # async fn example() -> Result<(), vectorless::BuildError> {
/// let engine = EngineBuilder::new()
///     .with_key("sk-...")
///     .with_model("gpt-4o")
///     .with_endpoint("https://api.openai.com/v1")
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// # Advanced usage (programmatic config)
///
/// ```rust,ignore
/// use vectorless::config::{Config, LlmConfig, SlotConfig};
///
/// let config = Config::new().with_llm(
///     LlmConfig::new("gpt-4o")
///         .with_api_key("sk-...")
///         .with_endpoint("https://api.openai.com/v1")
///         .with_index(SlotConfig::fast().with_model("gpt-4o-mini"))
///         .with_retrieval(SlotConfig::default().with_max_tokens(200))
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// API key — **required**.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Default model name — **required**.
    ///
    /// Individual slots can override this via [`SlotConfig::model`].
    #[serde(default)]
    pub model: String,

    /// API endpoint URL — **required**.
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Index slot (document indexing / summarization).
    /// Uses a fast, cost-effective model by default.
    #[serde(default)]
    pub index: SlotConfig,

    /// Retrieval slot (document navigation).
    /// Uses the default model.
    #[serde(default = "default_retrieval_slot")]
    pub retrieval: SlotConfig,

    /// Pilot slot (navigation guidance).
    /// Uses a fast model with higher token limit.
    #[serde(default = "default_pilot_slot")]
    pub pilot: SlotConfig,

    /// Retry configuration for LLM calls.
    #[serde(default)]
    pub retry: RetryConfig,

    /// Throttle / rate-limiting configuration.
    #[serde(default)]
    pub throttle: ThrottleConfig,

    /// Fallback configuration for error recovery.
    #[serde(default)]
    pub fallback: FallbackConfig,
}

fn default_retrieval_slot() -> SlotConfig {
    SlotConfig {
        max_tokens: 100,
        ..SlotConfig::default()
    }
}

fn default_pilot_slot() -> SlotConfig {
    SlotConfig {
        max_tokens: 300,
        ..SlotConfig::default()
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            model: String::new(),
            endpoint: None,
            index: SlotConfig::default(),
            retrieval: default_retrieval_slot(),
            pilot: default_pilot_slot(),
            retry: RetryConfig::default(),
            throttle: ThrottleConfig::default(),
            fallback: FallbackConfig::default(),
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

    /// Set the API key.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the default model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the endpoint URL.
    pub fn with_endpoint(mut self, url: impl Into<String>) -> Self {
        self.endpoint = Some(url.into());
        self
    }

    /// Set the index slot configuration.
    pub fn with_index(mut self, slot: SlotConfig) -> Self {
        self.index = slot;
        self
    }

    /// Set the retrieval slot configuration.
    pub fn with_retrieval(mut self, slot: SlotConfig) -> Self {
        self.retrieval = slot;
        self
    }

    /// Set the pilot slot configuration.
    pub fn with_pilot(mut self, slot: SlotConfig) -> Self {
        self.pilot = slot;
        self
    }

    /// Set the retry configuration.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    /// Set the throttle configuration.
    pub fn with_throttle(mut self, throttle: ThrottleConfig) -> Self {
        self.throttle = throttle;
        self
    }

    /// Set the fallback configuration.
    pub fn with_fallback(mut self, fallback: FallbackConfig) -> Self {
        self.fallback = fallback;
        self
    }

    /// Convenience: set max concurrent requests (delegates to throttle).
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.throttle.max_concurrent_requests = max;
        self
    }

    /// Resolve the effective model for a given slot.
    ///
    /// Returns the slot-specific model if set, otherwise the default model.
    pub fn resolve_model(&self, slot: &SlotConfig) -> String {
        slot.model.clone().unwrap_or_else(|| self.model.clone())
    }
}

/// Per-purpose LLM slot override.
///
/// Controls model selection and generation parameters for a specific
/// LLM usage (index, retrieval, or pilot).
///
/// - `model`: Override the default model (optional).
/// - `max_tokens`: Maximum response tokens.
/// - `temperature`: Generation temperature.
///
/// `api_key` and `endpoint` are **not** here — they are always inherited
/// from the parent [`LlmConfig`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotConfig {
    /// Override the default model for this purpose.
    /// When `None`, uses [`LlmConfig::model`].
    #[serde(default)]
    pub model: Option<String>,

    /// Maximum tokens for responses.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Temperature for generation.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_max_tokens() -> usize {
    200
}

fn default_temperature() -> f32 {
    0.0
}

impl Default for SlotConfig {
    fn default() -> Self {
        Self {
            model: None,
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
        }
    }
}

impl SlotConfig {
    /// Create a new slot config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a "fast" preset (low tokens).
    pub fn fast() -> Self {
        Self {
            max_tokens: 100,
            ..Self::default()
        }
    }

    /// Set the model override.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
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
}

// ============================================================
// Supporting configuration types
// ============================================================

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
        let delay_ms = (self.initial_delay_ms as f64) * self.multiplier.powi(attempt as i32);
        let delay_ms = delay_ms.min(self.max_delay_ms as f64);
        std::time::Duration::from_millis(delay_ms as u64)
    }

    /// Convert to the runtime retry config (used by llm module).
    pub fn to_runtime_config(&self) -> crate::llm::config::RetryConfig {
        crate::llm::config::RetryConfig {
            max_attempts: self.max_attempts,
            initial_delay_ms: self.initial_delay_ms,
            max_delay_ms: self.max_delay_ms,
            multiplier: self.multiplier,
            retry_on_rate_limit: self.retry_on_rate_limit,
        }
    }
}

/// Throttle / rate-limiting configuration.
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

    /// Convert to the runtime concurrency config.
    pub fn to_runtime_config(&self) -> crate::throttle::ConcurrencyConfig {
        crate::throttle::ConcurrencyConfig {
            max_concurrent_requests: self.max_concurrent_requests,
            requests_per_minute: self.requests_per_minute,
            enabled: self.enabled,
            semaphore_enabled: self.semaphore_enabled,
        }
    }
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

/// Fallback configuration for error recovery.
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

    /// Set behavior on rate limit.
    pub fn with_on_rate_limit(mut self, behavior: FallbackBehavior) -> Self {
        self.on_rate_limit = behavior;
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
    fn test_llm_config_defaults() {
        let config = LlmConfig::default();
        assert!(config.api_key.is_none());
        assert!(config.model.is_empty());
        assert!(config.endpoint.is_none());
        assert!(config.index.model.is_none());
        assert!(config.retrieval.model.is_none());
        assert!(config.pilot.model.is_none());
        assert_eq!(config.index.max_tokens, 200);
        assert_eq!(config.retrieval.max_tokens, 100);
        assert_eq!(config.pilot.max_tokens, 300);
    }

    #[test]
    fn test_llm_config_builder() {
        let config = LlmConfig::new("gpt-4o")
            .with_api_key("sk-test")
            .with_endpoint("https://api.openai.com/v1")
            .with_index(SlotConfig::fast().with_model("gpt-4o-mini"));

        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.api_key, Some("sk-test".to_string()));
        assert_eq!(config.index.model, Some("gpt-4o-mini".to_string()));
        assert_eq!(config.index.max_tokens, 100);
    }

    #[test]
    fn test_resolve_model() {
        let config =
            LlmConfig::new("gpt-4o").with_retrieval(SlotConfig::new().with_model("gpt-4o-mini"));

        assert_eq!(config.resolve_model(&config.index), "gpt-4o");
        assert_eq!(config.resolve_model(&config.retrieval), "gpt-4o-mini");
        assert_eq!(config.resolve_model(&config.pilot), "gpt-4o");
    }

    #[test]
    fn test_slot_config_fast() {
        let slot = SlotConfig::fast();
        assert_eq!(slot.max_tokens, 100);
    }

    #[test]
    fn test_retry_delay_calculation() {
        let config = RetryConfig::default();
        assert_eq!(
            config.delay_for_attempt(0),
            std::time::Duration::from_millis(500)
        );
        assert_eq!(
            config.delay_for_attempt(1),
            std::time::Duration::from_millis(1000)
        );
    }

    #[test]
    fn test_throttle_config_defaults() {
        let config = ThrottleConfig::default();
        assert_eq!(config.max_concurrent_requests, 10);
        assert_eq!(config.requests_per_minute, 500);
    }

    #[test]
    fn test_fallback_config_defaults() {
        let config = FallbackConfig::default();
        assert!(config.enabled);
        assert!(!config.models.is_empty());
    }
}
