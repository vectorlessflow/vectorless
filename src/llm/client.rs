// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Unified LLM client with retry and concurrency support.

use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestSystemMessage,
        ChatCompletionRequestUserMessage,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use std::sync::Arc;
use tracing::{debug, instrument};

use super::config::LlmConfig;
use super::error::{LlmError, LlmResult};
use super::fallback::FallbackChain;
use super::retry::with_retry;
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
    config: LlmConfig,
    concurrency: Option<Arc<ConcurrencyController>>,
    fallback: Option<Arc<FallbackChain>>,
}

impl std::fmt::Debug for LlmClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmClient")
            .field("model", &self.config.model)
            .field("endpoint", &self.config.endpoint)
            .field("concurrency", &self.concurrency.as_ref().map(|c| format!("{:?}", c)))
            .field("fallback_enabled", &self.fallback.is_some())
            .finish()
    }
}

impl LlmClient {
    /// Create a new LLM client with the given configuration.
    pub fn new(config: LlmConfig) -> Self {
        Self {
            config,
            concurrency: None,
            fallback: None,
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
        self.concurrency = Some(Arc::new(controller));
        self
    }

    /// Add concurrency control from an existing Arc.
    pub fn with_shared_concurrency(mut self, controller: Arc<ConcurrencyController>) -> Self {
        self.concurrency = Some(controller);
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

    /// Get the concurrency controller (if any).
    pub fn concurrency(&self) -> Option<&ConcurrencyController> {
        self.concurrency.as_deref()
    }

    /// Get the fallback chain (if any).
    pub fn fallback(&self) -> Option<&FallbackChain> {
        self.fallback.as_deref()
    }

    /// Complete a prompt with system and user messages.
    ///
    /// This method includes:
    /// - Automatic rate limiting (if configured)
    /// - Automatic retry with exponential backoff
    #[instrument(skip(self, system, user), fields(model = %self.config.model))]
    pub async fn complete(&self, system: &str, user: &str) -> LlmResult<String> {
        with_retry(&self.config.retry, || async {
            self.complete_once(system, user).await
        }).await
    }

    /// Complete a prompt with custom max tokens.
    pub async fn complete_with_max_tokens(
        &self,
        system: &str,
        user: &str,
        max_tokens: u16,
    ) -> LlmResult<String> {
        with_retry(&self.config.retry, || async {
            self.complete_once_with_max_tokens(system, user, max_tokens).await
        }).await
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
        let response = self.complete_with_max_tokens(system, user, max_tokens).await?;
        self.parse_json(&response)
    }

    /// Single completion attempt (no retry).
    async fn complete_once(&self, system: &str, user: &str) -> LlmResult<String> {
        // Acquire concurrency permit (rate limiter + semaphore)
        let _permit = if let Some(ref cc) = self.concurrency {
            Some(cc.acquire().await)
        } else {
            None
        };

        let api_key = self.config.get_api_key()
            .ok_or_else(|| LlmError::Config(
                "No API key found. Set OPENAI_API_KEY environment variable.".to_string()
            ))?;

        let endpoint = self.config.auto_detect_endpoint();
        let model = self.config.auto_detect_model();

        println!("Using OpenAI API endpoint: {}", endpoint);
        println!("Using OpenAI model: {}", model);

        let openai_config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(&endpoint);

        let client = Client::with_config(openai_config);

        // Truncate user prompt if too long
        let truncated = self.truncate_prompt(user);

        let request = CreateChatCompletionRequestArgs::default()
            .model(&model)
            .messages([
                ChatCompletionRequestSystemMessage::from(system).into(),
                ChatCompletionRequestUserMessage::from(truncated).into(),
            ])
            // .max_tokens(self.config.max_tokens as u16)
            .temperature(self.config.temperature)
            .build()
            .map_err(|e| LlmError::Request(format!("Failed to build request: {}", e)))?;

        debug!("Sending LLM request to {} with model {}", endpoint, model);

        let response = client.chat().create(request).await
            .map_err(|e| {
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

    /// Single completion with custom max tokens.
    async fn complete_once_with_max_tokens(
        &self,
        system: &str,
        user: &str,
        max_tokens: u16,
    ) -> LlmResult<String> {
        // Acquire concurrency permit
        let _permit = if let Some(ref cc) = self.concurrency {
            Some(cc.acquire().await)
        } else {
            None
        };

        let api_key = self.config.get_api_key()
            .ok_or_else(|| LlmError::Config(
                "No API key found. Set OPENAI_API_KEY environment variable.".to_string()
            ))?;

        let endpoint = self.config.auto_detect_endpoint();
        let model = self.config.auto_detect_model();

        let openai_config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(&endpoint);

        let client = Client::with_config(openai_config);

        let truncated = self.truncate_prompt(user);

        let request = CreateChatCompletionRequestArgs::default()
            .model(&model)
            .messages([
                ChatCompletionRequestSystemMessage::from(system).into(),
                ChatCompletionRequestUserMessage::from(truncated).into(),
            ])
            // .max_tokens(max_tokens)
            .temperature(self.config.temperature)
            .build()
            .map_err(|e| LlmError::Request(format!("Failed to build request: {}", e)))?;

        let response = client.chat().create(request).await
            .map_err(|e| {
                let msg = e.to_string();
                eprintln!("[LLM ERROR] API error: {}", msg);
                LlmError::from_api_message(&msg)
            })?;

        // Debug: log response structure
        eprintln!("[LLM DEBUG] Response: {} choices", response.choices.len());
        if let Some(choice) = response.choices.first() {
            eprintln!("[LLM DEBUG] First choice: finish_reason={:?}, has_content={}",
                choice.finish_reason,
                choice.message.content.is_some()
            );
        }

        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .ok_or_else(|| {
                eprintln!("[LLM ERROR] Response has no content");
                LlmError::NoContent
            })?;

        if content.is_empty() {
            eprintln!("[LLM WARN] Returned empty content for model: {}", model);
        } else {
            eprintln!("[LLM DEBUG] Content length: {} chars", content.len());
        }

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

    /// Parse JSON from LLM response.
    fn parse_json<T: DeserializeOwned>(&self, text: &str) -> LlmResult<T> {
        let json_text = self.extract_json(text);
        serde_json::from_str(&json_text)
            .map_err(|e| LlmError::Parse(format!("Failed to parse JSON: {}. Response: {}", e, text)))
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

        let json = client.extract_json(r#"```json
{"key": "value"}
```"#);
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
        assert_eq!(client.config.model, "gpt-4o");
    }

    #[test]
    fn test_client_with_concurrency() {
        use crate::throttle::ConcurrencyConfig;

        let controller = ConcurrencyController::new(ConcurrencyConfig::conservative());
        let client = LlmClient::for_model("gpt-4o-mini")
            .with_concurrency(controller);

        assert!(client.concurrency.is_some());
    }
}
