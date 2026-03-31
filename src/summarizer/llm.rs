// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM provider using async-openai.
//!
//! This module provides utilities for LLM-based text summarization.

use async_openai::{
    types::completions::CreateCompletionRequestArgs,
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

/// Generate a summary of the given text.
///
/// Uses the configured summary model for cost-effective indexing.
pub async fn summarize(
    config: &SummaryConfig,
    text: &str,
) -> Result<String, LlmError> {
    let api_key = config.api_key.as_ref()
        .ok_or_else(|| LlmError::Config("API key not configured".to_string()))?;

    let openai_config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(&config.endpoint);

    let client = Client::with_config(openai_config);

    let truncated = if text.len() > 8000 { &text[..8000] } else { text };

    let prompt = format!(
        "Summarize the following in 2-3 sentences. Be specific and factual. Do not add anything not in the text.\n\n{}",
        truncated
    );

    let request = CreateCompletionRequestArgs::default()
        .model(&config.model)
        .prompt(&prompt)
        // .max_tokens(max_tokens as u32)
        .build()
        .map_err(|e| LlmError::Request(e.to_string()))?;

    let response = client.completions().create(request).await?;
    let content = response
        .choices
        .first()
        .map(|c| c.text.trim().to_string())
        .unwrap_or_default();

    Ok(content)
}
