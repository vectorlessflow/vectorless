// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM provider using async-openai.
//!
//! This module provides utilities for LLM-based text processing,
//! supporting different models for indexing vs retrieval (cost optimization).

use crate::SummaryConfig;
use async_openai::{types::completions::CreateCompletionRequestArgs, Client, config::OpenAIConfig};

/// LLM error types.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    /// API error
    #[error("API error: {0}")]
    Api(String),
    /// Request construction error
    #[error("Request error: {0}")]
    Request(String),
}

/// Generate a summary of the given text.
///
/// Uses the configured summary model for cost-effective indexing.
pub async fn summarize(api_key: &str, config: &SummaryConfig, text: &str, max_tokens: u32) -> Result<String, LlmError> {
    let openai_config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(&config.endpoint);

    let client = Client::with_config(openai_config);

    let truncated = if text.len() > 3000 { &text[..3000] } else { text };

    let prompt = format!(
        "Summarize the following in 2-3 sentences. Be specific and factual. Do not add anything not in the text.\n\n{}",
        truncated
    );

    let request = CreateCompletionRequestArgs::default()
        .model(&config.model)
        .prompt(&prompt)
        .max_tokens(max_tokens as u32)
        .build()
        .map_err(|e| LlmError::Request(e.to_string()))?;

    let response = client
        .completions()
        .create(request)
        .await
        .map_err(|e| LlmError::Api(e.to_string()))?;

    let content = response
        .choices
        .first()
        .map(|c| c.text.trim().to_string())
        .unwrap_or_default();

    Ok(content)
}