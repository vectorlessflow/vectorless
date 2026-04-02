// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration type definitions.

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

// Indexer configuration.
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
}

/// Default subsection word count threshold.
pub fn default_subsection_threshold() -> usize {
    300
}

/// Default maximum tokens for segmentation.
pub fn default_max_segment_tokens() -> usize {
    3000
}

/// Default maximum tokens for summaries.
pub fn default_max_summary_tokens() -> usize {
    200
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            subsection_threshold: default_subsection_threshold(),
            max_segment_tokens: default_max_segment_tokens(),
            max_summary_tokens: default_max_summary_tokens(),
        }
    }
}

/// Generic LLM configuration.
///
/// Used for both summarization and retrieval/navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Model name (e.g., "gpt-4o-mini", "claude-3-haiku").
    #[serde(default = "default_llm_model")]
    pub model: String,

    /// API endpoint.
    #[serde(default = "default_llm_endpoint")]
    pub endpoint: String,

    /// API key (prefer using environment variable).
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for responses.
    #[serde(default = "default_llm_max_tokens")]
    pub max_tokens: usize,

    /// Temperature for generation.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

/// Default LLM model name.
fn default_llm_model() -> String {
    "gpt-4o-mini".to_string()
}

/// Default LLM API endpoint, auto-detected from environment.
fn default_llm_endpoint() -> String {
    // Auto-detect based on available API keys
    if std::env::var("OPENAI_API_KEY").is_ok() {
        "https://api.openai.com/v1".to_string()
    } else if std::env::var("AZURE_OPENAI_ENDPOINT").is_ok() {
        std::env::var("AZURE_OPENAI_ENDPOINT").unwrap_or_default()
    } else {
        "https://api.z.ai/api/paas/v4".to_string()
    }
}

/// Default maximum tokens for LLM responses.
fn default_llm_max_tokens() -> usize {
    1000
}

/// Default temperature for LLM generation.
pub(crate) fn default_temperature() -> f32 {
    0.0
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model: default_llm_model(),
            endpoint: default_llm_endpoint(),
            api_key: None,
            max_tokens: default_llm_max_tokens(),
            temperature: default_temperature(),
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

    /// Get the API key from config or environment.
    pub fn get_api_key(&self) -> Option<String> {
        self.api_key.clone().or_else(|| {
            if std::env::var("OPENAI_API_KEY").is_ok() {
                std::env::var("OPENAI_API_KEY").ok()
            } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
                std::env::var("ANTHROPIC_API_KEY").ok()
            } else {
                None
            }
        })
    }
}

/// Summary model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryConfig {
    /// Model name for summarization.
    #[serde(default = "default_summary_model")]
    pub model: String,

    /// API endpoint for summary model.
    #[serde(default = "default_summary_endpoint")]
    pub endpoint: String,

    /// API key (prefer using environment variable).
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for summary generation.
    #[serde(default = "default_summary_max_tokens")]
    pub max_tokens: usize,

    /// Temperature for summary generation.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

/// Default summary model name.
pub fn default_summary_model() -> String {
    // Auto-detect based on available API keys
    if std::env::var("OPENAI_API_KEY").is_ok() {
        "gpt-4o-mini".to_string()
    } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        "claude-3-sonnet-20240229".to_string()
    } else {
        "glm-5".to_string()
    }
}

/// Default summary endpoint, auto-detected from environment.
pub fn default_summary_endpoint() -> String {
    // Auto-detect based on available API keys
    if std::env::var("OPENAI_API_KEY").is_ok() {
        "https://api.openai.com/v1".to_string()
    } else if std::env::var("AZURE_OPENAI_ENDPOINT").is_ok() {
        std::env::var("AZURE_OPENAI_ENDPOINT").unwrap_or_default()
    } else {
        "https://api.z.ai/api/paas/v4".to_string()
    }
}

/// Default maximum tokens for summary generation.
pub fn default_summary_max_tokens() -> usize {
    200
}

impl Default for SummaryConfig {
    fn default() -> Self {
        Self {
            model: default_summary_model(),
            endpoint: default_summary_endpoint(),
            api_key: None,
            max_tokens: default_summary_max_tokens(),
            temperature: default_temperature(),
        }
    }
}

/// Retriever type enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrieverType {
    /// Adaptive retrieval (default).
    Adaptive,
}

impl Default for RetrieverType {
    fn default() -> Self {
        Self::Adaptive
    }
}

/// Retrieval model configuration (for navigation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalConfig {
    /// Model name for retrieval/navigation.
    #[serde(default = "default_retrieval_model")]
    pub model: String,

    /// API endpoint for retrieval model.
    #[serde(default = "default_retrieval_endpoint")]
    pub endpoint: String,

    /// API key (prefer using environment variable).
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for retrieval context.
    #[serde(default = "default_retrieval_max_tokens")]
    pub max_tokens: usize,

    /// Temperature for retrieval.
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Number of top-k results to return.
    #[serde(default = "default_top_k")]
    pub top_k: usize,

    /// Retriever type to use.
    #[serde(default)]
    pub retriever_type: RetrieverType,
}

/// Default retrieval model name.
pub fn default_retrieval_model() -> String {
    // Auto-detect based on available API keys
    if std::env::var("OPENAI_API_KEY").is_ok() {
        "gpt-4o".to_string()
    } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        "claude-3-sonnet-20240229".to_string()
    } else {
        "glm-5".to_string()
    }
}

/// Default retrieval endpoint, auto-detected from environment.
pub fn default_retrieval_endpoint() -> String {
    // Auto-detect based on available API keys
    if std::env::var("OPENAI_API_KEY").is_ok() {
        "https://api.openai.com/v1".to_string()
    } else if std::env::var("AZURE_OPENAI_ENDPOINT").is_ok() {
        std::env::var("AZURE_OPENAI_ENDPOINT").unwrap_or_default()
    } else {
        "https://api.z.ai/api/paas/v4".to_string()
    }
}

/// Default maximum tokens for retrieval context.
pub fn default_retrieval_max_tokens() -> usize {
    1000
}

/// Default number of top results to return.
pub fn default_top_k() -> usize {
    3
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            model: default_retrieval_model(),
            endpoint: default_retrieval_endpoint(),
            api_key: None,
            max_tokens: default_retrieval_max_tokens(),
            temperature: default_temperature(),
            top_k: default_top_k(),
            retriever_type: RetrieverType::default(),
        }
    }
}

/// Storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Workspace directory for persisted documents.
    ///
    /// Structure:
    /// ```text
    /// workspace/
    /// ├── _meta.json           # Lightweight index
    /// ├── {doc_id_1}.json      # Document 1
    /// └── {doc_id_2}.json      # Document 2
    /// ```
    #[serde(default = "default_workspace_dir")]
    pub workspace_dir: PathBuf,
}

/// Default workspace directory path.
pub fn default_workspace_dir() -> PathBuf {
    PathBuf::from("./workspace")
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            workspace_dir: default_workspace_dir(),
        }
    }
}

/// Concurrency control configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Maximum concurrent LLM API calls.
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,

    /// Rate limit: requests per minute.
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: usize,

    /// Whether rate limiting is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether semaphore-based concurrency limiting is enabled.
    #[serde(default = "default_true")]
    pub semaphore_enabled: bool,
}

/// Default maximum concurrent requests.
pub fn default_max_concurrent_requests() -> usize {
    10
}

/// Default requests per minute rate limit.
pub fn default_requests_per_minute() -> usize {
    500
}

/// Default boolean value (true).
pub fn default_true() -> bool {
    true
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: default_max_concurrent_requests(),
            requests_per_minute: default_requests_per_minute(),
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
    pub fn to_runtime_config(&self) -> crate::concurrency::ConcurrencyConfig {
        crate::concurrency::ConcurrencyConfig {
            max_concurrent_requests: self.max_concurrent_requests,
            requests_per_minute: self.requests_per_minute,
            enabled: self.enabled,
            semaphore_enabled: self.semaphore_enabled,
        }
    }
}

impl From<ConcurrencyConfig> for crate::concurrency::ConcurrencyConfig {
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
    /// Example: ["gpt-4o", "gpt-4o-mini", "glm-4-flash"]
    #[serde(default)]
    pub models: Vec<String>,

    /// Fallback endpoints in priority order.
    /// Example: ["https://api.openai.com/v1", "https://api.z.ai/api/paas/v4"]
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
            models: vec![
                "gpt-4o-mini".to_string(),
                "glm-4-flash".to_string(),
            ],
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
