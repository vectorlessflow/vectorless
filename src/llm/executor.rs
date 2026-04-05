// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Unified executor coordinating throttle, retry, and fallback.
//!
//! This module provides the `LlmExecutor` which coordinates:
//! - **Throttle** — Rate limiting and concurrency control
//! - **Retry** — Exponential backoff on transient errors
//! - **Fallback** — Model/endpoint degradation on persistent failures
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        LlmExecutor                               │
//! │                                                                  │
//! │   execute() ──▶ [Throttle] ──▶ [API Call] ──▶ [Success/Error]   │
//! │                       │              │                           │
//! │                  acquire permit    do request                    │
//! │                                      │                           │
//! │                           ┌──────────┴──────────┐               │
//! │                           ▼                     ▼                │
//! │                      [Retry]              [Fallback]             │
//! │                           │                     │                │
//! │                    exponential           model/endpoint          │
//! │                      backoff             degradation             │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::llm::{LlmExecutor, LlmConfig, FallbackChain, FallbackConfig};
//! use vectorless::throttle::{ConcurrencyController, ConcurrencyConfig};
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::llm::LlmResult<()> {
//! let config = LlmConfig::new("gpt-4o");
//! let throttle = ConcurrencyController::new(ConcurrencyConfig::default());
//! let fallback = FallbackChain::new(FallbackConfig::default());
//!
//! let executor = LlmExecutor::new(config)
//!     .with_throttle(throttle)
//!     .with_fallback(fallback);
//!
//! let result = executor.complete("You are helpful.", "Hello!").await?;
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

use super::config::LlmConfig;
use super::error::{LlmError, LlmResult};
use super::fallback::{FallbackChain, FallbackStep};
use crate::throttle::ConcurrencyController;

/// Unified executor for LLM operations.
///
/// Coordinates throttle, retry, and fallback mechanisms.
#[derive(Clone)]
pub struct LlmExecutor {
    /// LLM configuration.
    config: LlmConfig,
    /// Throttle controller (optional).
    throttle: Option<Arc<ConcurrencyController>>,
    /// Fallback chain (optional).
    fallback: Option<Arc<FallbackChain>>,
}

impl std::fmt::Debug for LlmExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmExecutor")
            .field("model", &self.config.model)
            .field("endpoint", &self.config.endpoint)
            .field("has_throttle", &self.throttle.is_some())
            .field("has_fallback", &self.fallback.is_some())
            .finish()
    }
}

impl LlmExecutor {
    /// Create a new executor with the given configuration.
    pub fn new(config: LlmConfig) -> Self {
        Self {
            config,
            throttle: None,
            fallback: None,
        }
    }

    /// Create an executor with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(LlmConfig::default())
    }

    /// Create an executor for a specific model.
    pub fn for_model(model: impl Into<String>) -> Self {
        Self::new(LlmConfig::new(model))
    }

    /// Add throttle control.
    pub fn with_throttle(mut self, controller: ConcurrencyController) -> Self {
        self.throttle = Some(Arc::new(controller));
        self
    }

    /// Add throttle control from an existing Arc.
    pub fn with_shared_throttle(mut self, controller: Arc<ConcurrencyController>) -> Self {
        self.throttle = Some(controller);
        self
    }

    /// Add fallback chain.
    pub fn with_fallback(mut self, chain: FallbackChain) -> Self {
        self.fallback = Some(Arc::new(chain));
        self
    }

    /// Add fallback chain from an existing Arc.
    pub fn with_shared_fallback(mut self, chain: Arc<FallbackChain>) -> Self {
        self.fallback = Some(chain);
        self
    }

    /// Get the configuration.
    pub fn config(&self) -> &LlmConfig {
        &self.config
    }

    /// Get the throttle controller (if any).
    pub fn throttle(&self) -> Option<&ConcurrencyController> {
        self.throttle.as_deref()
    }

    /// Get the fallback chain (if any).
    pub fn fallback(&self) -> Option<&FallbackChain> {
        self.fallback.as_deref()
    }

    /// Execute a completion with unified coordination.
    ///
    /// This method coordinates:
    /// 1. Throttle: Acquire permit before API call
    /// 2. Retry: Exponential backoff on transient errors
    /// 3. Fallback: Model/endpoint degradation on persistent failures
    pub async fn complete(&self, system: &str, user: &str) -> LlmResult<String> {
        self.execute_with_context(system, user, None).await
    }

    /// Execute a completion with custom max tokens.
    pub async fn complete_with_max_tokens(
        &self,
        system: &str,
        user: &str,
        max_tokens: u16,
    ) -> LlmResult<String> {
        self.execute_with_context(system, user, Some(max_tokens))
            .await
    }

    /// Internal execution with full coordination.
    async fn execute_with_context(
        &self,
        system: &str,
        user: &str,
        max_tokens: Option<u16>,
    ) -> LlmResult<String> {
        let mut attempts = 0;
        let mut current_model = self.config.model.clone();
        let current_endpoint = self.config.auto_detect_endpoint();
        let mut fallback_history: Vec<FallbackStep> = vec![];
        let mut total_attempts_including_fallback = 0;

        loop {
            attempts += 1;
            total_attempts_including_fallback += 1;

            // Safety check: prevent infinite loops
            const MAX_TOTAL_ATTEMPTS: usize = 20;
            if total_attempts_including_fallback > MAX_TOTAL_ATTEMPTS {
                warn!(
                    total_attempts = total_attempts_including_fallback,
                    "Exceeded maximum total attempts, aborting"
                );
                return Err(LlmError::RetryExhausted {
                    attempts: total_attempts_including_fallback,
                    last_error: "Exceeded maximum total attempts including fallbacks".to_string(),
                });
            }

            // Step 1: Acquire throttle permit
            let _permit = self.acquire_throttle_permit().await;

            debug!(
                attempt = attempts,
                model = %current_model,
                endpoint = %current_endpoint,
                "Executing LLM request"
            );

            // Step 2: Execute the request
            let result = self
                .do_request(&current_model, &current_endpoint, system, user, max_tokens)
                .await;

            match result {
                Ok(response) => {
                    if fallback_history.is_empty() {
                        debug!(
                            attempts = attempts,
                            "LLM request succeeded without fallback"
                        );
                    } else {
                        info!(
                            attempts = attempts,
                            fallback_steps = fallback_history.len(),
                            "LLM request succeeded after fallback"
                        );
                    }
                    return Ok(response);
                }
                Err(error) => {
                    // Step 3: Check if we should retry
                    if self.should_retry(&error, attempts) {
                        let delay = self.retry_delay(attempts);
                        warn!(
                            attempt = attempts,
                            max_attempts = self.config.retry.max_attempts,
                            delay_ms = delay.as_millis() as u64,
                            error = %error,
                            "LLM call failed, retrying..."
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    // Step 4: Check if we should fallback
                    if let Some(ref fallback) = self.fallback {
                        if fallback.should_fallback(&error) {
                            let mut fell_back = false;

                            // Try next model
                            if let Some(next_model) = fallback.next_model(&current_model) {
                                info!(
                                    from_model = %current_model,
                                    to_model = %next_model,
                                    "Falling back to next model"
                                );
                                fallback.record_fallback(
                                    &mut fallback_history,
                                    current_model.clone(),
                                    Some(next_model.clone()),
                                    current_endpoint.clone(),
                                    None,
                                    error.to_string(),
                                );
                                current_model = next_model;
                                attempts = 0; // Reset retry counter for new model
                                fell_back = true;
                            }

                            if fell_back {
                                continue;
                            }
                        }
                    }

                    // Step 5: No more retries or fallbacks, return error
                    warn!(
                        attempts = attempts,
                        fallback_steps = fallback_history.len(),
                        error = %error,
                        "LLM call failed, no more retries or fallbacks available"
                    );
                    return Err(error);
                }
            }
        }
    }

    /// Acquire throttle permit (if configured).
    async fn acquire_throttle_permit(&self) -> Option<tokio::sync::SemaphorePermit<'_>> {
        if let Some(ref throttle) = self.throttle {
            throttle.acquire().await
        } else {
            None
        }
    }

    /// Check if we should retry based on error and attempt count.
    fn should_retry(&self, error: &LlmError, attempts: usize) -> bool {
        if attempts >= self.config.retry.max_attempts {
            return false;
        }

        match error {
            LlmError::RateLimit(_) => self.config.retry.retry_on_rate_limit,
            LlmError::Timeout(_) => true,
            LlmError::Api(msg) => {
                let msg_lower = msg.to_lowercase();
                msg_lower.contains("rate limit")
                    || msg_lower.contains("429")
                    || msg_lower.contains("503")
                    || msg_lower.contains("502")
                    || msg_lower.contains("timeout")
                    || msg_lower.contains("overloaded")
            }
            _ => false,
        }
    }

    /// Calculate retry delay for a given attempt.
    fn retry_delay(&self, attempt: usize) -> Duration {
        self.config.retry.delay_for_attempt(attempt - 1)
    }

    /// Execute the actual API request.
    async fn do_request(
        &self,
        model: &str,
        endpoint: &str,
        system: &str,
        user: &str,
        max_tokens: Option<u16>,
    ) -> LlmResult<String> {
        use async_openai::{
            Client,
            config::OpenAIConfig,
            types::chat::{
                ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage,
                CreateChatCompletionRequestArgs,
            },
        };

        let api_key = self.config.get_api_key().ok_or_else(|| {
            LlmError::Config(
                "No API key found. Set OPENAI_API_KEY environment variable.".to_string(),
            )
        })?;

        let openai_config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(endpoint);

        let client = Client::with_config(openai_config);

        // Truncate user prompt if too long
        let truncated = self.truncate_prompt(user);

        // Build request based on whether max_tokens is specified
        let request = if let Some(tokens) = max_tokens {
            CreateChatCompletionRequestArgs::default()
                .model(model)
                .messages([
                    ChatCompletionRequestSystemMessage::from(system).into(),
                    ChatCompletionRequestUserMessage::from(truncated).into(),
                ])
                .temperature(self.config.temperature)
                .max_tokens(tokens)
                .build()
        } else {
            CreateChatCompletionRequestArgs::default()
                .model(model)
                .messages([
                    ChatCompletionRequestSystemMessage::from(system).into(),
                    ChatCompletionRequestUserMessage::from(truncated).into(),
                ])
                .temperature(self.config.temperature)
                .build()
        };

        let request =
            request.map_err(|e| LlmError::Request(format!("Failed to build request: {}", e)))?;

        debug!("Sending LLM request to {} with model {}", endpoint, model);

        let response = client.chat().create(request).await.map_err(|e| {
            let msg = e.to_string();
            LlmError::from_api_message(&msg)
        })?;

        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .ok_or(LlmError::NoContent)?;

        debug!("LLM response length: {} chars", content.len());

        Ok(content)
    }

    /// Truncate a prompt to a reasonable length.
    fn truncate_prompt<'a>(&self, text: &'a str) -> &'a str {
        // Roughly 4 chars per token, limit to ~30k chars
        const MAX_CHARS: usize = 30000;
        if text.len() > MAX_CHARS {
            &text[..MAX_CHARS]
        } else {
            text
        }
    }
}

impl Default for LlmExecutor {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_creation() {
        let executor = LlmExecutor::for_model("gpt-4o");
        assert_eq!(executor.config().model, "gpt-4o");
        assert!(executor.throttle().is_none());
        assert!(executor.fallback().is_none());
    }

    #[test]
    fn test_executor_with_throttle() {
        use crate::throttle::ConcurrencyConfig;

        let controller = ConcurrencyController::new(ConcurrencyConfig::conservative());
        let executor = LlmExecutor::for_model("gpt-4o-mini").with_throttle(controller);

        assert!(executor.throttle().is_some());
    }

    #[test]
    fn test_should_retry() {
        let executor = LlmExecutor::with_defaults();

        // Should retry on timeout
        assert!(executor.should_retry(&LlmError::Timeout("test".to_string()), 1));

        // Should retry on rate limit (if configured)
        assert!(executor.should_retry(&LlmError::RateLimit("test".to_string()), 1));

        // Should not retry on config error
        assert!(!executor.should_retry(&LlmError::Config("test".to_string()), 1));

        // Should not retry after max attempts
        assert!(!executor.should_retry(&LlmError::Timeout("test".to_string()), 100));
    }

    #[test]
    fn test_retry_delay() {
        let executor = LlmExecutor::with_defaults();

        // First retry attempt (attempt 1 -> delay_for_attempt(0))
        let delay = executor.retry_delay(1);
        assert_eq!(delay, Duration::from_millis(500));
    }
}
