// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Builder pattern for creating Engine clients.
//!
//! This module provides [`EngineBuilder`] for configuring and building
//! [`Engine`] instances with sensible defaults.
//!
//! # Configuration
//!
//! `api_key` and `model` are **required**. `endpoint` is optional
//! (defaults to the model provider's standard endpoint).
//!
//! Configuration sources (later overrides earlier):
//! 1. Default configuration
//! 2. Config file (via `with_config_path`)
//! 3. Builder methods (`with_key`, `with_model`, etc.) — highest priority
//!
//! # Examples
//!
//! ```rust,no_run
//! use vectorless::client::EngineBuilder;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), vectorless::BuildError> {
//! let engine = EngineBuilder::new()
//!     .with_key("sk-...")
//!     .with_model("gpt-4o")
//!     .build()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## With Custom Endpoint
//!
//! ```rust,no_run
//! use vectorless::client::EngineBuilder;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), vectorless::BuildError> {
//! let engine = EngineBuilder::new()
//!     .with_key("sk-...")
//!     .with_model("deepseek-chat")
//!     .with_endpoint("https://api.deepseek.com/v1")
//!     .build()
//!     .await?;
//! # Ok(())
//! # }
//! ```

use crate::config::{Config, ConfigLoader, RetrievalConfig};
use crate::memo::MemoStore;
use crate::retrieval::PipelineRetriever;
use crate::storage::Workspace;

use super::engine::Engine;
use crate::events::EventEmitter;

/// Builder for creating a [`Engine`] client.
///
/// `api_key` and `model` are required and must be set via builder methods
/// or provided through a config file.
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::client::EngineBuilder;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), vectorless::BuildError> {
/// let client = EngineBuilder::new()
///     .with_key("sk-...")
///     .with_model("gpt-4o")
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct EngineBuilder {
    /// Configuration file path.
    config_path: Option<std::path::PathBuf>,

    /// Custom configuration.
    config: Option<Config>,

    /// Custom retrieval config.
    retrieval_config: Option<RetrievalConfig>,

    /// Event emitter.
    events: Option<EventEmitter>,

    /// LLM API key (override).
    api_key: Option<String>,

    /// LLM model name (override).
    model: Option<String>,

    /// LLM endpoint URL (override).
    endpoint: Option<String>,

    /// Top-K for retrieval (override).
    top_k: Option<usize>,

    /// Fast mode flag.
    fast_mode: bool,

    /// Precise mode flag.
    precise_mode: bool,

    /// Memo store for caching LLM decisions.
    memo_store: Option<MemoStore>,
}

impl EngineBuilder {
    /// Create a new builder with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config_path: None,
            config: None,
            retrieval_config: None,
            events: None,
            api_key: None,
            model: None,
            endpoint: None,
            top_k: None,
            fast_mode: false,
            precise_mode: false,
            memo_store: None,
        }
    }

    // ============================================================
    // Basic Configuration
    // ============================================================

    /// Set the configuration file path.
    ///
    /// The file must be a valid TOML configuration. No auto-detection is performed.
    #[must_use]
    pub fn with_config_path(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// Set a custom configuration object.
    ///
    /// This overrides any config file settings.
    #[must_use]
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    /// Set custom retrieval configuration.
    #[must_use]
    pub fn with_retrieval_config(mut self, config: RetrievalConfig) -> Self {
        self.retrieval_config = Some(config);
        self
    }

    /// Set the event emitter for callbacks.
    #[must_use]
    pub fn with_events(mut self, events: EventEmitter) -> Self {
        self.events = Some(events);
        self
    }

    /// Set a memo store for caching LLM decisions.
    ///
    /// When enabled, the pilot will cache navigation decisions based on
    /// context fingerprints, avoiding redundant API calls for similar
    /// navigation scenarios.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vectorless::client::EngineBuilder;
    /// use vectorless::memo::MemoStore;
    /// use chrono::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), vectorless::BuildError> {
    /// let memo_store = MemoStore::new()
    ///     .with_ttl(Duration::days(7))
    ///     .with_model("gpt-4o");
    ///
    /// let engine = EngineBuilder::new()
    ///     .with_key("sk-...")
    ///     .with_model("gpt-4o")
    ///     .with_memo_store(memo_store)
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_memo_store(mut self, store: MemoStore) -> Self {
        self.memo_store = Some(store);
        self
    }

    // ============================================================
    // LLM Configuration
    // ============================================================

    /// Set the LLM API key. **Required**.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vectorless::client::EngineBuilder;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), vectorless::BuildError> {
    /// let engine = EngineBuilder::new()
    ///     .with_key("sk-...")
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the LLM model name.
    ///
    /// Default: "gpt-4o".
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vectorless::client::EngineBuilder;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), vectorless::BuildError> {
    /// let engine = EngineBuilder::new()
    ///     .with_model("gpt-4o-mini")
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set a custom LLM endpoint URL.
    ///
    /// Use this for OpenAI-compatible APIs (e.g., Azure OpenAI, local models).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vectorless::client::EngineBuilder;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), vectorless::BuildError> {
    /// let engine = EngineBuilder::new()
    ///     .with_model("deepseek-chat")
    ///     .with_endpoint("https://api.deepseek.com/v1")
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_endpoint(mut self, url: impl Into<String>) -> Self {
        self.endpoint = Some(url.into());
        self
    }

    // ============================================================
    // Retrieval Configuration
    // ============================================================

    /// Set the number of results to return from queries.
    ///
    /// Default is 5. Higher values return more context but cost more tokens.
    #[must_use]
    pub fn with_top_k(mut self, k: usize) -> Self {
        self.top_k = Some(k);
        self
    }

    // ============================================================
    // Preset Configurations
    // ============================================================

    /// Enable fast mode for quicker but less thorough retrieval.
    ///
    /// Fast mode uses:
    /// - Keyword-based retrieval (no LLM calls)
    /// - Lower beam width / MCTS simulations
    /// - Lazy summary generation
    #[must_use]
    pub fn fast(mut self) -> Self {
        self.fast_mode = true;
        self.precise_mode = false;
        self
    }

    /// Enable precise mode for higher quality retrieval.
    ///
    /// Precise mode uses:
    /// - MCTS-based retrieval
    /// - Higher simulation count
    /// - Full summary generation
    #[must_use]
    pub fn precise(mut self) -> Self {
        self.precise_mode = true;
        self.fast_mode = false;
        self
    }

    /// Build the Engine client.
    ///
    /// `api_key` and `model` must be provided via builder methods or config file.
    ///
    /// # Errors
    ///
    /// Returns a [`BuildError`] if:
    /// - Configuration loading fails
    /// - Workspace creation fails
    /// - Required `api_key` or `model` is missing
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vectorless::client::EngineBuilder;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), vectorless::BuildError> {
    /// let engine = EngineBuilder::new()
    ///     .with_key("sk-...")
    ///     .with_model("gpt-4o")
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn build(self) -> Result<Engine, BuildError> {
        // Load or create configuration
        let mut config = if let Some(config) = self.config {
            config
        } else if let Some(path) = self.config_path {
            ConfigLoader::new()
                .file(&path)
                .load()
                .map_err(|e| BuildError::Config(e.to_string()))?
        } else {
            Config::default()
        };

        // Apply builder overrides to retrieval config
        if let Some(retrieval_config) = self.retrieval_config {
            config.retrieval = retrieval_config;
        }

        // Apply individual overrides to LlmPoolConfig (primary) + legacy config (compat)
        if let Some(api_key) = self.api_key {
            config.llm.api_key = Some(api_key.clone());
            // Legacy compat
            config.retrieval.api_key = Some(api_key.clone());
            config.summary.api_key = Some(api_key);
        }
        if let Some(model) = self.model {
            // Apply model to pool slots
            if config.llm.index.model.is_empty() {
                config.llm.index.model = model.clone();
            }
            if config.llm.retrieval.model.is_empty() {
                config.llm.retrieval.model = model.clone();
            }
            if config.llm.pilot.model.is_empty() {
                config.llm.pilot.model = model.clone();
            }
            // Legacy compat
            config.retrieval.model = model.clone();
            config.summary.model = model;
        }
        if let Some(endpoint) = self.endpoint {
            config.llm.endpoint = Some(endpoint.clone());
            // Legacy compat
            config.retrieval.endpoint = endpoint.clone();
            config.summary.endpoint = endpoint;
        }
        if let Some(top_k) = self.top_k {
            config.retrieval.top_k = top_k;
        }

        // Apply preset modes
        if self.fast_mode {
            config.retrieval.search.max_iterations = 5;
        }
        if self.precise_mode {
            config.retrieval.search.max_iterations = 100;
        }

        // Validate required settings
        let resolved_key = config
            .llm
            .api_key
            .as_ref()
            .or_else(|| config.llm.retrieval.api_key.as_ref())
            .or_else(|| config.summary.api_key.as_ref())
            .or_else(|| config.retrieval.api_key.as_ref());
        if resolved_key.is_none() {
            return Err(BuildError::MissingApiKey);
        }
        let retrieval_model = if config.llm.retrieval.model.is_empty() {
            &config.retrieval.model
        } else {
            &config.llm.retrieval.model
        };
        if retrieval_model.is_empty() {
            return Err(BuildError::MissingModel);
        }

        // Open workspace from config
        let workspace = Workspace::new(&config.storage.workspace_dir)
            .await
            .map_err(|e| BuildError::Workspace(e.to_string()))?;

        // Build LlmPool from config.llm — centralizes all LLM client creation
        let llm_configs: crate::llm::LlmConfigs = config.llm.clone().into();
        let pool = {
            let controller = crate::throttle::ConcurrencyController::new(
                crate::throttle::ConcurrencyConfig::new()
                    .with_max_concurrent_requests(config.concurrency.max_concurrent_requests)
                    .with_requests_per_minute(config.concurrency.requests_per_minute)
                    .with_enabled(config.concurrency.enabled),
            );
            crate::llm::LlmPool::new(llm_configs).with_concurrency(controller)
        };

        // Indexer uses pool.index()
        let indexer = crate::client::indexer::IndexerClient::with_llm(pool.index().clone());

        // Retriever uses pool.retrieval()
        let retrieval_config = config.retrieval.clone();
        let mut retriever =
            PipelineRetriever::new().with_max_iterations(retrieval_config.search.max_iterations);
        retriever = retriever.with_llm_client(pool.retrieval().clone());

        // Configure content aggregator if enabled
        if retrieval_config.content.enabled {
            retriever =
                retriever.with_content_config(retrieval_config.content.to_aggregator_config());
        }

        // Add memo store if provided or create default
        if let Some(memo_store) = self.memo_store {
            retriever = retriever.with_memo_store(memo_store);
        } else {
            // Create default memo store with model from config
            let memo_store = MemoStore::new().with_model(retrieval_model).with_version(1);
            retriever = retriever.with_memo_store(memo_store);
        }

        // Build engine
        let events = self.events.unwrap_or_default();
        Engine::with_components(config, workspace, retriever, indexer, events)
            .await
            .map_err(|e| BuildError::Other(e.to_string()))
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Error during client build.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Workspace error.
    #[error("Workspace error: {0}")]
    Workspace(String),

    /// Missing API key.
    #[error("Missing API key: call .with_key(\"sk-...\") or set api_key in config file")]
    MissingApiKey,

    /// Missing model name.
    #[error("Missing model: call .with_model(\"gpt-4o\") or set model in config file")]
    MissingModel,

    /// Other error.
    #[error("{0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let builder = EngineBuilder::new();
        assert!(!builder.fast_mode);
        assert!(!builder.precise_mode);
    }

    #[test]
    fn test_builder_with_key() {
        let builder = EngineBuilder::new().with_key("sk-test-key");

        assert_eq!(builder.api_key, Some("sk-test-key".to_string()));
    }

    #[test]
    fn test_builder_with_model() {
        let builder = EngineBuilder::new().with_model("gpt-4o-mini");

        assert_eq!(builder.model, Some("gpt-4o-mini".to_string()));
    }

    #[test]
    fn test_builder_with_key_and_model() {
        let builder = EngineBuilder::new()
            .with_model("gpt-4o-mini")
            .with_key("sk-test");

        assert_eq!(builder.model, Some("gpt-4o-mini".to_string()));
        assert_eq!(builder.api_key, Some("sk-test".to_string()));
    }

    #[test]
    fn test_builder_fast_mode() {
        let builder = EngineBuilder::new().fast();

        assert!(builder.fast_mode);
        assert!(!builder.precise_mode);
    }

    #[test]
    fn test_builder_precise_mode() {
        let builder = EngineBuilder::new().precise();

        assert!(builder.precise_mode);
        assert!(!builder.fast_mode);
    }

    #[test]
    fn test_builder_top_k() {
        let builder = EngineBuilder::new().with_top_k(10);

        assert_eq!(builder.top_k, Some(10));
    }
}
