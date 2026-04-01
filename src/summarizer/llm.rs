// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM provider using async-openai.
//!
//! This module provides utilities for LLM-based text summarization.
//! API keys are automatically detected from environment variables.

use async_openai::{
    types::chat::{ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage, CreateChatCompletionRequestArgs},
    Client,
    config::OpenAIConfig,
    error::OpenAIError,
};
use thiserror::Error;
use crate::config::SummaryConfig;

/// LLM error types.
#[derive(Debug, Error)]
pub enum LlmError {
    /// API error
    #[error("API error: {0}")]
    Api(String),

    /// Request construction error
    #[error("Request error: {0}")]
    Request(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),
}

impl From<OpenAIError> for LlmError {
    fn from(e: OpenAIError) -> Self {
        LlmError::Api(e.to_string())
    }
}

/// Get API key from environment variables.
///
/// Checks in order: OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.
fn get_api_key_from_env() -> Option<String> {
    std::env::var("OPENAI_API_KEY").ok()
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .or_else(|| std::env::var("AZURE_OPENAI_API_KEY").ok())
}

/// Get API base URL from environment variables.
fn get_api_base_from_env() -> Option<String> {
    std::env::var("OPENAI_API_BASE").ok()
        .or_else(|| std::env::var("OPENAI_BASE_URL").ok())
        .or_else(|| std::env::var("AZURE_OPENAI_ENDPOINT").ok())
}

/// Generate a summary of the given text.
///
/// Uses the configured summary model for cost-effective indexing.
/// API key is automatically detected from environment variables if not configured.
pub async fn summarize(
    config: &SummaryConfig,
    text: &str,
) -> Result<String, LlmError> {
    // Use config API key or fall back to environment
    let api_key = config.api_key.as_ref()
        .cloned()
        .or_else(get_api_key_from_env)
        .ok_or_else(|| LlmError::Config(
            "No API key found. Set OPENAI_API_KEY environment variable or configure in SummaryConfig.".to_string()
        ))?;

    // Use config endpoint or fall back to environment
    let api_base = if config.endpoint.is_empty() || config.endpoint == "https://api.openai.com/v1" {
        get_api_base_from_env().unwrap_or_else(|| config.endpoint.clone())
    } else {
        config.endpoint.clone()
    };

    let openai_config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(&api_base);
    let client = Client::with_config(openai_config);

    let truncated = if text.len() > 8000 { &text[..8000] } else { text };

    let request = CreateChatCompletionRequestArgs::default()
        .model(&config.model)
        .messages(
            [
                // System message: define the task and behavior
                ChatCompletionRequestSystemMessage::from(
                    "You are a helpful assistant that summarizes text concisely and accurately. \
                     Summarize the user's text in 2-3 sentences. Be specific and factual. \
                     Do not add anything not in the original text."
                ).into(),
                // User message: the actual content to summarize
                ChatCompletionRequestUserMessage::from(truncated).into(),
            ]
        )
        .temperature(1.0)
        .build()
        .map_err(|e| LlmError::Request(e.to_string()))?;

    let response = client.chat().create(request).await?;
    let content = response
        .choices
        .first()
        .map(|choice| choice.message.content.clone())
        .unwrap_or_default();

    content.ok_or_else(|| LlmError::Api("LLM returned no content".to_string()))
}
