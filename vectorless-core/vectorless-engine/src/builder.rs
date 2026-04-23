// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Builder pattern for creating Engine clients.
//!
//! This module provides [`EngineBuilder`] for configuring and building
//! [`Engine`] instances with sensible defaults.

use vectorless_config::Config;
use vectorless_events::EventEmitter;
use vectorless_metrics::MetricsHub;
use vectorless_storage::Workspace;

use super::engine::Engine;
use super::retriever::RetrieverClient;

/// Builder for creating a [`Engine`] client.
///
/// `api_key`, `model` and `endpoint` are **required** for simple usage.
/// Advanced users can provide a pre-built [`Config`] via [`with_config`](EngineBuilder::with_config).
///
/// # Example (simple)
///
/// ```rust,no_run
/// use vectorless::client::EngineBuilder;
///
/// #[tokio::main]
/// async fn main() -> Result<(), vectorless::BuildError> {
///     let client = EngineBuilder::new()
///         .with_key("sk-...")
///         .with_model("gpt-4o")
///         .with_endpoint("https://api.xxx.com/v1")
///         .build()
///         .await?;
///    Ok(())
/// }
/// ```
///
/// # Example (advanced)
///
/// ```rust,ignore
/// use vectorless::client::EngineBuilder;
/// use vectorless::config::{Config, LlmConfig, SlotConfig};
///
/// let config = Config::new().with_llm(
///     LlmConfig::new("gpt-4o")
///         .with_api_key("sk-...")
///         .with_endpoint("https://api.openai.com/v1")
///         .with_index(SlotConfig::fast().with_model("gpt-4o-mini"))
/// );
///
/// let engine = EngineBuilder::new()
///     .with_config(config)
///     .build()
///     .await?;
/// ```
#[derive(Debug)]
pub struct EngineBuilder {
    /// Custom configuration for advanced tuning.
    config: Option<Config>,

    /// Event emitter.
    events: Option<EventEmitter>,

    /// LLM API key (override).
    api_key: Option<String>,

    /// LLM model name (override).
    model: Option<String>,

    /// LLM endpoint URL (override).
    endpoint: Option<String>,
}

impl EngineBuilder {
    /// Create a new builder with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: None,
            events: None,
            api_key: None,
            model: None,
            endpoint: None,
        }
    }

    // ============================================================
    // Configuration
    // ============================================================

    /// Set a custom configuration.
    ///
    /// When provided, this replaces the default [`Config`] entirely.
    /// Builder methods (`with_key`, `with_model`, `with_endpoint`)
    /// will still override the corresponding fields on top of this config.
    #[must_use]
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the event emitter for callbacks.
    #[must_use]
    pub fn with_events(mut self, events: EventEmitter) -> Self {
        self.events = Some(events);
        self
    }

    // ============================================================
    // LLM Configuration (simple overrides)
    // ============================================================

    /// Set the LLM API key. **Required** (unless provided via Config).
    #[must_use]
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the LLM model name.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set a custom LLM endpoint URL.
    #[must_use]
    pub fn with_endpoint(mut self, url: impl Into<String>) -> Self {
        self.endpoint = Some(url.into());
        self
    }

    // ============================================================
    // Build
    // ============================================================

    /// Build the Engine client.
    ///
    /// # Errors
    ///
    /// Returns a [`BuildError`] if:
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
    ///     .with_endpoint("https://api.openai.com/v1")
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn build(self) -> Result<Engine, BuildError> {
        // Load user-provided or default configuration
        let mut config = self.config.unwrap_or_default();

        // Apply simple overrides — write once, no dual-writing
        if let Some(api_key) = self.api_key {
            config.llm.api_key = Some(api_key);
        }
        if let Some(model) = self.model {
            config.llm.model = model;
        }
        if let Some(endpoint) = self.endpoint {
            config.llm.endpoint = Some(endpoint);
        }

        // Validate required settings
        if config.llm.api_key.is_none() {
            return Err(BuildError::MissingApiKey);
        }
        if config.llm.model.is_empty() {
            return Err(BuildError::MissingModel);
        }
        if config.llm.endpoint.is_none() {
            return Err(BuildError::MissingEndpoint);
        }

        // Open workspace from config
        let workspace = Workspace::new(&config.storage.workspace_dir)
            .await
            .map_err(|e| BuildError::Workspace(e.to_string()))?;

        // Build LlmPool from unified LlmConfig (shared metrics hub)
        let metrics_hub = std::sync::Arc::new(MetricsHub::with_defaults());
        let pool = vectorless_llm::LlmPool::from_config(&config.llm, Some(metrics_hub.clone()));

        // Indexer uses pool.index()
        let indexer = super::indexer::IndexerClient::with_llm(pool.index().clone());

        // Retriever uses pool.retrieval() via agent system
        let retriever = RetrieverClient::new(pool.retrieval().clone());

        // Build engine
        let events = self.events.unwrap_or_default();
        Engine::with_components(config, workspace, retriever, indexer, events, metrics_hub)
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
    /// Workspace error.
    #[error("Workspace error: {0}")]
    Workspace(String),

    /// Missing API key.
    #[error("Missing API key: call .with_key(\"sk-...\") or set api_key in config")]
    MissingApiKey,

    /// Missing model name.
    #[error("Missing model: call .with_model(\"gpt-4o\") or set model in config")]
    MissingModel,

    /// Missing endpoint URL.
    #[error(
        "Missing endpoint: call .with_endpoint(\"https://api.xxx.com/v1\") or set endpoint in config"
    )]
    MissingEndpoint,

    /// Other error.
    #[error("{0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
