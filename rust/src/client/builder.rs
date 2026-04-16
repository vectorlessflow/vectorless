// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Builder pattern for creating Engine clients.
//!
//! This module provides [`EngineBuilder`] for configuring and building
//! [`Engine`] instances with sensible defaults.

use crate::{
    client::engine::Engine, config::Config, events::EventEmitter, retrieval::PipelineRetriever,
    storage::Workspace,
};

/// Builder for creating a [`Engine`] client.
///
/// `api_key`, `model` and `endpoint` are **required**.
///
/// # Example
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
    // Basic Configuration
    // ============================================================

    /// Set a custom configuration for advanced tuning of internal parameters.
    ///
    /// When provided, this replaces the default [`Config`]. Builder methods
    /// (`with_key`, `with_model`, `with_endpoint`) still override the
    /// corresponding fields.
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
        // Load user-provided or default configuration
        let mut config = self.config.unwrap_or_default();

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
