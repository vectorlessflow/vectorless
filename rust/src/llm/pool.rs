// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM client pool for managing multiple clients.

use std::sync::Arc;

use super::client::LlmClient;
use super::config::LlmConfig;
use super::fallback::{FallbackChain, FallbackConfig};
use crate::metrics::MetricsHub;
use super::throttle::ConcurrencyController;

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
}

impl LlmPool {
    /// Create a pool from the unified LLM configuration.
    ///
    /// Resolves per-slot model overrides and creates individual
    /// [`LlmClient`] instances with the appropriate settings.
    /// When `metrics` is provided, all clients share the same hub
    /// for unified LLM call statistics.
    pub fn from_config(
        config: &crate::config::LlmConfig,
        metrics: Option<Arc<MetricsHub>>,
    ) -> Self {
        let api_key = config.api_key.clone();
        let endpoint = config.endpoint.clone().unwrap_or_default();
        let retry = config.retry.to_runtime_config();

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

        // Build a single shared async-openai client (reuses connection pool)
        let openai_base = if endpoint.is_empty() {
            "https://api.openai.com/v1".to_string()
        } else {
            endpoint.clone()
        };
        let openai_client = Arc::new(async_openai::Client::with_config(
            async_openai::config::OpenAIConfig::new()
                .with_api_key(api_key.clone().unwrap_or_default())
                .with_api_base(openai_base),
        ));

        // Attach shared throttle controller from config
        let concurrency_config = config.throttle.to_runtime_config();
        let controller = Arc::new(ConcurrencyController::new(concurrency_config));

        // Attach shared fallback chain from config
        let fallback_config: FallbackConfig = config.fallback.clone().into();
        let fallback_chain = Arc::new(FallbackChain::new(fallback_config));

        let build_client = |slot_config: &crate::config::SlotConfig| {
            let mut client = LlmClient::new(make_config(slot_config))
                .with_shared_concurrency(controller.clone())
                .with_shared_openai_client(openai_client.clone())
                .with_shared_fallback(fallback_chain.clone());
            if let Some(ref hub) = metrics {
                client = client.with_shared_metrics(hub.clone());
            }
            Arc::new(client)
        };

        Self {
            index: build_client(&config.index),
            retrieval: build_client(&config.retrieval),
            pilot: build_client(&config.pilot),
        }
    }

    /// Create a pool with default configurations.
    pub fn from_defaults() -> Self {
        Self::from_config(&crate::config::LlmConfig::default(), None)
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
    fn test_pool_from_config() {
        let config = crate::config::LlmConfig::new("gpt-4o")
            .with_api_key("sk-test")
            .with_endpoint("https://api.openai.com/v1")
            .with_index(crate::config::SlotConfig::fast().with_model("gpt-4o-mini"));

        let pool = LlmPool::from_config(&config, None);

        assert_eq!(pool.index().config().model, "gpt-4o-mini");
        assert_eq!(pool.retrieval().config().model, "gpt-4o");
        assert_eq!(pool.pilot().config().model, "gpt-4o");
        assert_eq!(pool.index().config().max_tokens, 100);
    }

    #[test]
    fn test_pool_from_config_with_metrics() {
        let config = crate::config::LlmConfig::new("gpt-4o")
            .with_api_key("sk-test")
            .with_endpoint("https://api.openai.com/v1");

        let hub = MetricsHub::shared();
        let pool = LlmPool::from_config(&config, Some(hub.clone()));

        // Verify each client has fallback (which means executor was built correctly)
        assert!(pool.index().fallback().is_some());
        assert!(pool.retrieval().fallback().is_some());
        assert!(pool.pilot().fallback().is_some());

        // Verify models are resolved correctly
        assert_eq!(pool.index().config().model, "gpt-4o");
        assert_eq!(pool.retrieval().config().model, "gpt-4o");
        assert_eq!(pool.pilot().config().model, "gpt-4o");
    }

    #[test]
    fn test_pool_shared_metrics_hub() {
        let config = crate::config::LlmConfig::new("gpt-4o")
            .with_api_key("sk-test")
            .with_endpoint("https://api.openai.com/v1");

        let hub = MetricsHub::shared();
        let _pool = LlmPool::from_config(&config, Some(hub.clone()));

        // Hub is shared with all three clients — Arc refcount > 1
        assert!(Arc::strong_count(&hub) > 1);
    }
}
