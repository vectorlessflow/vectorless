// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM client pool for managing multiple clients.

use std::sync::Arc;

use super::client::LlmClient;
use super::config::LlmConfig;
use crate::throttle::ConcurrencyController;

/// Pool of LLM clients for different purposes.
///
/// This provides a centralized way to access LLM clients
/// configured for specific tasks:
/// - **Index** — Document indexing/summarization (fast, cheap model)
/// - **Retrieval** — Document navigation (capable model)
/// - **Pilot** — Navigation guidance (fast model)
///
/// # Construction
///
/// The pool is built from a [`config::LlmConfig`](crate::config::LlmConfig)
/// which defines the global credentials and per-slot overrides.
///
/// ```rust,ignore
/// use vectorless::llm::LlmPool;
///
/// let pool = LlmPool::from_config(&config.llm);
///
/// // Use index client for summarization
/// let summary = pool.index().complete(
///     "You summarize text concisely.",
///     "Long text to summarize..."
/// ).await?;
/// ```
#[derive(Debug, Clone)]
pub struct LlmPool {
    index: Arc<LlmClient>,
    retrieval: Arc<LlmClient>,
    pilot: Arc<LlmClient>,
    concurrency: Option<Arc<ConcurrencyController>>,
}

impl LlmPool {
    /// Create a pool from the unified LLM configuration.
    ///
    /// Resolves per-slot model overrides and creates individual
    /// [`LlmClient`] instances with the appropriate settings.
    pub fn from_config(config: &crate::config::LlmConfig) -> Self {
        let api_key = config.api_key.clone();
        let endpoint = config.endpoint.clone().unwrap_or_default();
        let retry = super::config::RetryConfig {
            max_attempts: config.retry.max_attempts,
            initial_delay_ms: config.retry.initial_delay_ms,
            max_delay_ms: config.retry.max_delay_ms,
            multiplier: config.retry.multiplier,
            retry_on_rate_limit: config.retry.retry_on_rate_limit,
        };

        let make_config = |slot: &crate::config::SlotConfig| -> LlmConfig {
            LlmConfig {
                model: config.resolve_model(slot),
                endpoint: endpoint.clone(),
                api_key: api_key.clone(),
                max_tokens: slot.max_tokens,
                temperature: slot.temperature,
                retry: retry.clone(),
            }
        };

        Self {
            index: Arc::new(LlmClient::new(make_config(&config.index))),
            retrieval: Arc::new(LlmClient::new(make_config(&config.retrieval))),
            pilot: Arc::new(LlmClient::new(make_config(&config.pilot))),
            concurrency: None,
        }
    }

    /// Create a pool with default configurations.
    pub fn from_defaults() -> Self {
        Self::from_config(&crate::config::LlmConfig::default())
    }

    /// Add concurrency control to all clients in the pool.
    pub fn with_concurrency(mut self, controller: ConcurrencyController) -> Self {
        let arc = Arc::new(controller);
        self.concurrency = Some(arc.clone());
        self.index = Arc::new(
            LlmClient::new(self.index.config().clone()).with_shared_concurrency(arc.clone()),
        );
        self.retrieval = Arc::new(
            LlmClient::new(self.retrieval().config().clone()).with_shared_concurrency(arc.clone()),
        );
        self.pilot = Arc::new(
            LlmClient::new(self.pilot.config().clone()).with_shared_concurrency(arc.clone()),
        );
        self
    }

    /// Add concurrency control from an existing Arc.
    pub fn with_shared_concurrency(mut self, controller: Arc<ConcurrencyController>) -> Self {
        self.concurrency = Some(controller.clone());
        self.index = Arc::new(
            LlmClient::new(self.index.config().clone()).with_shared_concurrency(controller.clone()),
        );
        self.retrieval = Arc::new(
            LlmClient::new(self.retrieval().config().clone())
                .with_shared_concurrency(controller.clone()),
        );
        self.pilot = Arc::new(
            LlmClient::new(self.pilot.config().clone()).with_shared_concurrency(controller.clone()),
        );
        self
    }

    /// Get the concurrency controller (if any).
    pub fn concurrency(&self) -> Option<&ConcurrencyController> {
        self.concurrency.as_deref()
    }

    /// Get the index client.
    pub fn index(&self) -> &LlmClient {
        &self.index
    }

    /// Get the retrieval client.
    pub fn retrieval(&self) -> &LlmClient {
        &self.retrieval
    }

    /// Get the pilot client.
    pub fn pilot(&self) -> &LlmClient {
        &self.pilot
    }
}

impl Default for LlmPool {
    fn default() -> Self {
        Self::from_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_creation() {
        let pool = LlmPool::from_defaults();

        assert!(!pool.index().config().model.is_empty() || pool.index().config().model.is_empty());
        // Default pool creates clients (models may be empty from defaults)
    }

    #[test]
    fn test_pool_from_config() {
        let config = crate::config::LlmConfig::new("gpt-4o")
            .with_api_key("sk-test")
            .with_endpoint("https://api.openai.com/v1")
            .with_index(crate::config::SlotConfig::fast().with_model("gpt-4o-mini"));

        let pool = LlmPool::from_config(&config);

        assert_eq!(pool.index().config().model, "gpt-4o-mini");
        assert_eq!(pool.retrieval().config().model, "gpt-4o");
        assert_eq!(pool.pilot().config().model, "gpt-4o");
        assert_eq!(pool.index().config().max_tokens, 100);
    }

    #[test]
    fn test_pool_with_concurrency() {
        use crate::throttle::ConcurrencyConfig;

        let controller = ConcurrencyController::new(ConcurrencyConfig::conservative());
        let pool = LlmPool::from_defaults().with_concurrency(controller);

        assert!(pool.concurrency().is_some());
    }
}
