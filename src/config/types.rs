// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration type definitions.
//!
//! All configuration values are defined inline in `Default` trait implementations.
//! Configuration is loaded from TOML files only - no environment variable magic.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration for vectorless.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Indexer configuration.
    #[serde(default)]
    pub indexer: IndexerConfig,

    /// Summary model configuration.
    #[serde(default)]
    pub summary: SummaryConfig,

    /// Retrieval model configuration.
    #[serde(default)]
    pub retrieval: RetrievalConfig,

    /// Storage configuration.
    #[serde(default)]
    pub storage: StorageConfig,

    /// Concurrency control configuration.
    #[serde(default)]
    pub concurrency: ConcurrencyConfig,

    /// Fallback/error recovery configuration.
    #[serde(default)]
    pub fallback: FallbackConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            indexer: IndexerConfig::default(),
            summary: SummaryConfig::default(),
            retrieval: RetrievalConfig::default(),
            storage: StorageConfig::default(),
            concurrency: ConcurrencyConfig::default(),
            fallback: FallbackConfig::default(),
        }
    }
}

/// Indexer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerConfig {
    /// Word count threshold for splitting sections into subsections.
    #[serde(default)]
    pub subsection_threshold: usize,

    /// Maximum tokens to send in a single segmentation request.
    #[serde(default)]
    pub max_segment_tokens: usize,

    /// Maximum tokens for each summary.
    #[serde(default)]
    pub max_summary_tokens: usize,

    /// Minimum content tokens required to generate a summary.
    #[serde(default)]
    pub min_summary_tokens: usize,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            subsection_threshold: 300,
            max_segment_tokens: 3000,
            max_summary_tokens: 200,
            min_summary_tokens: 20,
        }
    }
}

/// Generic LLM configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Model name (e.g., "gpt-4o-mini", "claude-3-haiku").
    #[serde(default)]
    pub model: String,

    /// API endpoint.
    #[serde(default)]
    pub endpoint: String,

    /// API key.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for responses.
    #[serde(default)]
    pub max_tokens: usize,

    /// Temperature for generation.
    #[serde(default)]
    pub temperature: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model: "gpt-4o-mini".to_string(),
            endpoint: "https://api.openai.com/v1".to_string(),
            api_key: None,
            max_tokens: 1000,
            temperature: 0.0,
        }
    }
}

impl LlmConfig {
    /// Create a new LLM config with defaults.
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

    /// Get the API key from config.
    pub fn get_api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }
}

/// Summary model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryConfig {
    /// Model name for summarization.
    #[serde(default)]
    pub model: String,

    /// API endpoint for summary model.
    #[serde(default)]
    pub endpoint: String,

    /// API key.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for summary generation.
    #[serde(default)]
    pub max_tokens: usize,

    /// Temperature for summary generation.
    #[serde(default)]
    pub temperature: f32,
}

impl Default for SummaryConfig {
    fn default() -> Self {
        Self {
            model: "gpt-4o-mini".to_string(),
            endpoint: "https://api.openai.com/v1".to_string(),
            api_key: None,
            max_tokens: 200,
            temperature: 0.0,
        }
    }
}

/// Retrieval model configuration (for navigation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalConfig {
    /// Model name for retrieval/navigation.
    #[serde(default)]
    pub model: String,

    /// API endpoint for retrieval model.
    #[serde(default)]
    pub endpoint: String,

    /// API key.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for retrieval context.
    #[serde(default)]
    pub max_tokens: usize,

    /// Temperature for retrieval.
    #[serde(default)]
    pub temperature: f32,

    /// Number of top-k results to return.
    #[serde(default)]
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
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            model: "gpt-4o".to_string(),
            endpoint: "https://api.openai.com/v1".to_string(),
            api_key: None,
            max_tokens: 1000,
            temperature: 0.0,
            top_k: 3,
            search: SearchConfig::default(),
            sufficiency: SufficiencyConfig::default(),
            cache: CacheConfig::default(),
            strategy: StrategyConfig::default(),
        }
    }
}

/// Search algorithm configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Number of top-k results to return.
    #[serde(default)]
    pub top_k: usize,

    /// Beam width for multi-path search.
    #[serde(default)]
    pub beam_width: usize,

    /// Maximum iterations for search algorithms.
    #[serde(default)]
    pub max_iterations: usize,

    /// Minimum score to include a path.
    #[serde(default)]
    pub min_score: f32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            top_k: 5,
            beam_width: 3,
            max_iterations: 10,
            min_score: 0.1,
        }
    }
}

/// Sufficiency checker configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SufficiencyConfig {
    /// Minimum tokens for sufficiency.
    #[serde(default)]
    pub min_tokens: usize,

    /// Target tokens for full sufficiency.
    #[serde(default)]
    pub target_tokens: usize,

    /// Maximum tokens before stopping.
    #[serde(default)]
    pub max_tokens: usize,

    /// Minimum content length (characters).
    #[serde(default)]
    pub min_content_length: usize,

    /// Confidence threshold for LLM judge.
    #[serde(default)]
    pub confidence_threshold: f32,
}

impl Default for SufficiencyConfig {
    fn default() -> Self {
        Self {
            min_tokens: 500,
            target_tokens: 2000,
            max_tokens: 4000,
            min_content_length: 200,
            confidence_threshold: 0.7,
        }
    }
}

/// Cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum number of cache entries.
    #[serde(default)]
    pub max_entries: usize,

    /// Time-to-live for cache entries (seconds).
    #[serde(default)]
    pub ttl_secs: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            ttl_secs: 3600,
        }
    }
}

/// Strategy-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// MCTS exploration weight (sqrt(2) ≈ 1.414).
    #[serde(default)]
    pub exploration_weight: f32,

    /// Semantic similarity threshold.
    #[serde(default)]
    pub similarity_threshold: f32,

    /// High similarity threshold for "answer" decision.
    #[serde(default)]
    pub high_similarity_threshold: f32,

    /// Low similarity threshold for "explore" decision.
    #[serde(default)]
    pub low_similarity_threshold: f32,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            exploration_weight: 1.414,
            similarity_threshold: 0.5,
            high_similarity_threshold: 0.8,
            low_similarity_threshold: 0.3,
        }
    }
}

/// Storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Workspace directory for persisted documents.
    #[serde(default)]
    pub workspace_dir: PathBuf,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            workspace_dir: PathBuf::from("./workspace"),
        }
    }
}

/// Concurrency control configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Maximum concurrent LLM API calls.
    #[serde(default)]
    pub max_concurrent_requests: usize,

    /// Rate limit: requests per minute.
    #[serde(default)]
    pub requests_per_minute: usize,

    /// Whether rate limiting is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether semaphore-based concurrency limiting is enabled.
    #[serde(default = "default_true")]
    pub semaphore_enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 10,
            requests_per_minute: 500,
            enabled: true,
            semaphore_enabled: true,
        }
    }
}

impl ConcurrencyConfig {
    /// Create a new config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum concurrent requests.
    pub fn with_max_concurrent_requests(mut self, max: usize) -> Self {
        self.max_concurrent_requests = max;
        self
    }

    /// Set the requests per minute rate limit.
    pub fn with_requests_per_minute(mut self, rpm: usize) -> Self {
        self.requests_per_minute = rpm;
        self
    }

    /// Enable or disable rate limiting.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Enable or disable semaphore.
    pub fn with_semaphore_enabled(mut self, enabled: bool) -> Self {
        self.semaphore_enabled = enabled;
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

impl From<ConcurrencyConfig> for crate::throttle::ConcurrencyConfig {
    fn from(config: ConcurrencyConfig) -> Self {
        config.to_runtime_config()
    }
}

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
    #[serde(default)]
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
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            models: vec!["gpt-4o-mini".to_string(), "glm-4-flash".to_string()],
            endpoints: vec![],
            on_rate_limit: FallbackBehavior::RetryThenFallback,
            on_timeout: FallbackBehavior::RetryThenFallback,
            on_all_failed: OnAllFailedBehavior::ReturnError,
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
}
