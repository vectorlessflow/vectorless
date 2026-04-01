// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM-based text summarization.
//!
//! This module provides utilities for LLM-based text summarization
//! using the unified [`crate::llm::LlmClient`].

use crate::config::SummaryConfig;
use crate::llm::{LlmClient, LlmConfig, LlmError};

/// Generate a summary of the given text.
///
/// Uses the configured summary model for cost-effective indexing.
/// API key is automatically detected from environment variables if not configured.
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::config::SummaryConfig;
/// use vectorless::summarizer::summarize;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), vectorless::summarizer::LlmError> {
/// let config = SummaryConfig::default();
/// let summary = summarize(&config, "Long text to summarize...").await?;
/// println!("Summary: {}", summary);
/// # Ok(())
/// # }
/// ```
pub async fn summarize(
    config: &SummaryConfig,
    text: &str,
) -> Result<String, LlmError> {
    // Convert SummaryConfig to LlmConfig
    let llm_config: LlmConfig = config.clone().into();
    let client = LlmClient::new(llm_config);

    let truncated = if text.len() > 8000 { &text[..8000] } else { text };

    client.complete(
        "You are a helpful assistant that summarizes text concisely and accurately. \
         Summarize the user's text in 2-3 sentences. Be specific and factual. \
         Do not add anything not in the original text.",
        truncated,
    ).await
}
