// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Unified LLM client with retry and concurrency support.

use serde::de::DeserializeOwned;
use std::borrow::Cow;
use std::sync::Arc;
use tracing::{debug, instrument};

use super::config::LlmConfig;
use super::error::{LlmError, LlmResult};
use super::executor::LlmExecutor;
use super::fallback::FallbackChain;
use crate::throttle::ConcurrencyController;

/// Unified LLM client.
///
/// This client provides:
/// - Unified interface for all LLM operations
/// - Automatic retry with exponential backoff
/// - Rate limiting and concurrency control
/// - JSON response parsing
/// - Error classification
/// - Graceful fallback on errors
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::llm::{LlmClient, LlmConfig};
///
/// # #[tokio::main]
/// # async fn main() -> vectorless::llm::LlmResult<()> {
/// let config = LlmConfig::new("gpt-4o-mini");
/// let client = LlmClient::new(config);
///
/// // Simple completion
/// let response = client.complete("You are helpful.", "Hello!").await?;
/// println!("Response: {}", response);
///
/// // JSON completion
/// #[derive(serde::Deserialize)]
/// struct Answer {
///     answer: String,
/// }
/// let answer: Answer = client.complete_json(
///     "You answer questions in JSON.",
///     "What is 2+2?"
/// ).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct LlmClient {
    executor: LlmExecutor,
}

impl std::fmt::Debug for LlmClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmClient")
            .field("model", &self.executor.config().model)
            .field("endpoint", &self.executor.config().endpoint)
            .field(
                "concurrency",
                &self.executor.throttle().map(|c| format!("{:?}", c)),
            )
            .field("fallback_enabled", &self.executor.fallback().is_some())
            .finish()
    }
}

impl LlmClient {
    /// Create a new LLM client with the given configuration.
    pub fn new(config: LlmConfig) -> Self {
        Self {
            executor: LlmExecutor::new(config),
        }
    }

    /// Create a client with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(LlmConfig::default())
    }

    /// Create a client for a specific model.
    pub fn for_model(model: impl Into<String>) -> Self {
        Self::new(LlmConfig::new(model))
    }

    /// Add concurrency control to the client.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use vectorless::llm::LlmClient;
    /// use vectorless::throttle::{ConcurrencyController, ConcurrencyConfig};
    ///
    /// let config = ConcurrencyConfig::new()
    ///     .with_max_concurrent_requests(10)
    ///     .with_requests_per_minute(500);
    ///
    /// let client = LlmClient::for_model("gpt-4o-mini")
    ///     .with_concurrency(ConcurrencyController::new(config));
    /// ```
    pub fn with_concurrency(mut self, controller: ConcurrencyController) -> Self {
        self.executor = self.executor.with_throttle(controller);
        self
    }

    /// Add concurrency control from an existing Arc.
    pub fn with_shared_concurrency(mut self, controller: Arc<ConcurrencyController>) -> Self {
        self.executor = self.executor.with_shared_throttle(controller);
        self
    }

    /// Replace the async-openai client with a shared instance (reuses connection pool).
    pub fn with_shared_openai_client(
        mut self,
        client: Arc<async_openai::Client<async_openai::config::OpenAIConfig>>,
    ) -> Self {
        self.executor = self.executor.with_openai_client(client);
        self
    }

    /// Add fallback chain for error recovery.
    ///
    /// # Example
    ///
    /// ```rust
    /// use vectorless::llm::{LlmClient, FallbackChain, FallbackConfig};
    ///
    /// let fallback = FallbackConfig::default();
    /// let client = LlmClient::for_model("gpt-4o")
    ///     .with_fallback(FallbackChain::new(fallback));
    ///
    /// assert!(client.fallback().is_some());
    /// ```
    pub fn with_fallback(mut self, chain: FallbackChain) -> Self {
        self.executor = self.executor.with_fallback(chain);
        self
    }

    /// Add fallback chain from an existing Arc.
    pub fn with_shared_fallback(mut self, chain: Arc<FallbackChain>) -> Self {
        self.executor = self.executor.with_shared_fallback(chain);
        self
    }

    /// Get the configuration.
    pub fn config(&self) -> &LlmConfig {
        self.executor.config()
    }

    /// Get the concurrency controller (if any).
    pub fn concurrency(&self) -> Option<&ConcurrencyController> {
        self.executor.throttle()
    }

    /// Get the fallback chain (if any).
    pub fn fallback(&self) -> Option<&FallbackChain> {
        self.executor.fallback()
    }

    /// Get the underlying executor (for advanced usage).
    pub fn executor(&self) -> &LlmExecutor {
        &self.executor
    }

    /// Complete a prompt with system and user messages.
    ///
    /// This method includes:
    /// - Automatic rate limiting (if configured)
    /// - Automatic retry with exponential backoff
    /// - Automatic fallback on persistent errors (if configured)
    #[instrument(skip(self, system, user), fields(model = %self.executor.config().model))]
    pub async fn complete(&self, system: &str, user: &str) -> LlmResult<String> {
        debug!(
            system_len = system.len(),
            user_len = user.len(),
            "Starting LLM completion"
        );
        self.executor.complete(system, user).await
    }

    /// Complete a prompt with custom max tokens.
    pub async fn complete_with_max_tokens(
        &self,
        system: &str,
        user: &str,
        max_tokens: u16,
    ) -> LlmResult<String> {
        debug!(
            system_len = system.len(),
            user_len = user.len(),
            max_tokens = max_tokens,
            "Starting LLM completion with max tokens"
        );
        self.executor
            .complete_with_max_tokens(system, user, max_tokens)
            .await
    }

    /// Complete a prompt and parse the response as JSON.
    ///
    /// This method handles:
    /// - JSON extraction from markdown code blocks
    /// - Bracket matching for nested JSON
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use vectorless::llm::{LlmClient, LlmConfig};
    /// # #[tokio::main]
    /// # async fn main() -> vectorless::llm::LlmResult<()> {
    /// #[derive(serde::Deserialize)]
    /// struct TocEntry {
    ///     title: String,
    ///     page: usize,
    /// }
    ///
    /// let client = LlmClient::for_model("gpt-4o-mini");
    /// let entries: Vec<TocEntry> = client.complete_json(
    ///     "Extract TOC entries as JSON array.",
    ///     "Chapter 1: Introduction ... 5"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn complete_json<T: DeserializeOwned>(
        &self,
        system: &str,
        user: &str,
    ) -> LlmResult<T> {
        let response = self.complete(system, user).await?;
        self.parse_json(&response)
    }

    /// Complete a prompt and parse the response as JSON with custom max tokens.
    pub async fn complete_json_with_max_tokens<T: DeserializeOwned>(
        &self,
        system: &str,
        user: &str,
        max_tokens: u16,
    ) -> LlmResult<T> {
        let response = self
            .complete_with_max_tokens(system, user, max_tokens)
            .await?;
        self.parse_json(&response)
    }

    /// Parse JSON from LLM response.
    fn parse_json<T: DeserializeOwned>(&self, text: &str) -> LlmResult<T> {
        let json_text = self.extract_json(text);
        serde_json::from_str(&json_text).map_err(|e| {
            LlmError::Parse(format!("Failed to parse JSON: {}. Response: {}", e, text))
        })
    }

    /// Extract JSON from text (handles markdown code blocks).
    fn extract_json<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let text = text.trim();

        // Try markdown code block first
        if text.starts_with("```") {
            // Find the end of the first line (language identifier)
            if let Some(start) = text.find('\n') {
                let rest = &text[start + 1..];
                if let Some(end) = rest.find("```") {
                    return Cow::Borrowed(rest[..end].trim());
                }
            }
        }

        // Try to find JSON array or object
        if text.starts_with('[') || text.starts_with('{') {
            let open = text.chars().next().unwrap();
            let close = if open == '[' { ']' } else { '}' };

            let mut depth = 0;
            for (i, ch) in text.char_indices() {
                match ch {
                    c if c == open => depth += 1,
                    c if c == close => {
                        depth -= 1;
                        if depth == 0 {
                            return Cow::Borrowed(&text[..=i]);
                        }
                    }
                    _ => {}
                }
            }
        }

        Cow::Borrowed(text)
    }
}

impl Default for LlmClient {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_plain() {
        let client = LlmClient::with_defaults();

        let json = client.extract_json(r#"{"key": "value"}"#);
        assert_eq!(json, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_code_block() {
        let client = LlmClient::with_defaults();

        let json = client.extract_json(
            r#"```json
{"key": "value"}
```"#,
        );
        assert_eq!(json, r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_array() {
        let client = LlmClient::with_defaults();

        let json = client.extract_json(r#"[1, 2, 3]"#);
        assert_eq!(json, r#"[1, 2, 3]"#);
    }

    #[test]
    fn test_extract_json_nested() {
        let client = LlmClient::with_defaults();

        let json = client.extract_json(r#"{"outer": {"inner": 1}}"#);
        assert_eq!(json, r#"{"outer": {"inner": 1}}"#);
    }

    #[test]
    fn test_client_creation() {
        let client = LlmClient::for_model("gpt-4o");
        assert_eq!(client.config().model, "gpt-4o");
    }

    #[test]
    fn test_client_with_concurrency() {
        use crate::throttle::ConcurrencyConfig;

        let controller = ConcurrencyController::new(ConcurrencyConfig::conservative());
        let client = LlmClient::for_model("gpt-4o-mini").with_concurrency(controller);

        assert!(client.concurrency().is_some());
    }
}
