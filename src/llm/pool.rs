// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM client pool for managing multiple clients.

use std::sync::Arc;

use super::client::LlmClient;
use super::config::LlmConfigs;
use crate::concurrency::ConcurrencyController;

/// Pool of LLM clients for different purposes.
///
/// This provides a centralized way to access LLM clients
/// configured for specific tasks:
/// - **Summary** — Document summarization (fast, cheap model)
/// - **Retrieval** — Document navigation (capable model)
/// - **TOC** — Table of contents processing (fast, cheap model)
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::llm::LlmPool;
///
/// # #[tokio::main]
/// # async fn main() -> vectorless::llm::LlmResult<()> {
/// let pool = LlmPool::from_defaults();
///
/// // Use summary client for summarization
/// let summary = pool.summary().complete(
///     "You summarize text concisely.",
///     "Long text to summarize..."
/// ).await?;
///
/// // Use retrieval client for navigation
/// let nav = pool.retrieval().complete(
///     "You navigate documents.",
///     "Find information about X..."
/// ).await?;
///
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct LlmPool {
    summary: Arc<LlmClient>,
    retrieval: Arc<LlmClient>,
    toc: Arc<LlmClient>,
    concurrency: Option<Arc<ConcurrencyController>>,
}

impl LlmPool {
    /// Create a new LLM pool from configurations.
    pub fn new(configs: LlmConfigs) -> Self {
        Self {
            summary: Arc::new(LlmClient::new(configs.summary)),
            retrieval: Arc::new(LlmClient::new(configs.retrieval)),
            toc: Arc::new(LlmClient::new(configs.toc)),
            concurrency: None,
        }
    }

    /// Create a pool with default configurations.
    ///
    /// Uses auto-detected models based on available API keys:
    /// - OpenAI: gpt-4o-mini for summary/toc, gpt-4o for retrieval
    /// - Anthropic: claude-3-haiku for summary/toc, claude-3-sonnet for retrieval
    /// - Default: glm-4-flash for summary/toc, glm-4 for retrieval
    pub fn from_defaults() -> Self {
        Self::new(LlmConfigs::default())
    }

    /// Add concurrency control to all clients in the pool.
    ///
    /// All clients share the same ConcurrencyController, which means
    /// rate limiting and concurrency limits are applied globally
    /// across all LLM operations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vectorless::llm::LlmPool;
    /// use vectorless::concurrency::{ConcurrencyController, ConcurrencyConfig};
    ///
    /// let config = ConcurrencyConfig::new()
    ///     .with_max_concurrent_requests(10)
    ///     .with_requests_per_minute(500);
    ///
    /// let pool = LlmPool::from_defaults()
    ///     .with_concurrency(ConcurrencyController::new(config));
    /// ```
    pub fn with_concurrency(mut self, controller: ConcurrencyController) -> Self {
        let arc = Arc::new(controller);
        self.concurrency = Some(arc.clone());
        self.summary = Arc::new(
            LlmClient::new(self.summary.config().clone())
                .with_shared_concurrency(arc.clone())
        );
        self.retrieval = Arc::new(
            LlmClient::new(self.retrieval.config().clone())
                .with_shared_concurrency(arc.clone())
        );
        self.toc = Arc::new(
            LlmClient::new(self.toc.config().clone())
                .with_shared_concurrency(arc.clone())
        );
        self
    }

    /// Add concurrency control from an existing Arc.
    pub fn with_shared_concurrency(mut self, controller: Arc<ConcurrencyController>) -> Self {
        self.concurrency = Some(controller.clone());
        self.summary = Arc::new(
            LlmClient::new(self.summary.config().clone())
                .with_shared_concurrency(controller.clone())
        );
        self.retrieval = Arc::new(
            LlmClient::new(self.retrieval.config().clone())
                .with_shared_concurrency(controller.clone())
        );
        self.toc = Arc::new(
            LlmClient::new(self.toc.config().clone())
                .with_shared_concurrency(controller.clone())
        );
        self
    }

    /// Get the concurrency controller (if any).
    pub fn concurrency(&self) -> Option<&ConcurrencyController> {
        self.concurrency.as_deref()
    }

    /// Get the summary client.
    ///
    /// Used for generating summaries of document sections.
    /// Typically uses a fast, cost-effective model.
    pub fn summary(&self) -> &LlmClient {
        &self.summary
    }

    /// Get the retrieval client.
    ///
    /// Used for document navigation and retrieval.
    /// Typically uses a more capable model for better navigation decisions.
    pub fn retrieval(&self) -> &LlmClient {
        &self.retrieval
    }

    /// Get the TOC client.
    ///
    /// Used for TOC detection, parsing, and page assignment.
    /// Typically uses a fast, cost-effective model.
    pub fn toc(&self) -> &LlmClient {
        &self.toc
    }

    /// Get a client for a specific purpose by name.
    ///
    /// # Arguments
    ///
    /// * `purpose` - One of: "summary", "summarize", "retrieval", "retrieve", "navigate", "toc"
    ///
    /// # Returns
    ///
    /// Returns `None` if the purpose is not recognized.
    pub fn get(&self, purpose: &str) -> Option<&LlmClient> {
        match purpose {
            "summary" | "summarize" => Some(&self.summary),
            "retrieval" | "retrieve" | "navigate" => Some(&self.retrieval),
            "toc" => Some(&self.toc),
            _ => None,
        }
    }

    /// Create a pool with a single model for all purposes.
    ///
    /// Useful for testing or simple deployments.
    pub fn single_model(model: impl Into<String>) -> Self {
        let config = super::config::LlmConfig::new(model);
        let client = Arc::new(LlmClient::new(config));
        Self {
            summary: client.clone(),
            retrieval: client.clone(),
            toc: client,
            concurrency: None,
        }
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

        // Should have all clients
        assert!(pool.get("summary").is_some());
        assert!(pool.get("retrieval").is_some());
        assert!(pool.get("toc").is_some());
        assert!(pool.get("unknown").is_none());
    }

    #[test]
    fn test_pool_get_aliases() {
        let pool = LlmPool::from_defaults();

        // Test aliases
        assert!(pool.get("summarize").is_some());
        assert!(pool.get("retrieve").is_some());
        assert!(pool.get("navigate").is_some());
    }

    #[test]
    fn test_single_model_pool() {
        let pool = LlmPool::single_model("gpt-4o-mini");

        // All clients should use the same model
        assert_eq!(pool.summary().config().model, "gpt-4o-mini");
        assert_eq!(pool.retrieval().config().model, "gpt-4o-mini");
        assert_eq!(pool.toc().config().model, "gpt-4o-mini");
    }

    #[test]
    fn test_pool_with_concurrency() {
        use crate::concurrency::ConcurrencyConfig;

        let controller = ConcurrencyController::new(ConcurrencyConfig::conservative());
        let pool = LlmPool::from_defaults().with_concurrency(controller);

        // All clients should have concurrency enabled
        assert!(pool.concurrency().is_some());
        assert!(pool.summary().concurrency().is_some());
        assert!(pool.retrieval().concurrency().is_some());
        assert!(pool.toc().concurrency().is_some());
    }
}
