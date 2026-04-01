// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM client utilities for TOC processing.

use async_openai::{
    types::chat::{
        ChatCompletionRequestSystemMessage,
        ChatCompletionRequestUserMessage,
        CreateChatCompletionRequestArgs,
    },
    Client,
    config::OpenAIConfig,
};

use crate::config::LlmConfig;
use crate::core::{Error, Result};

/// LLM client wrapper for TOC operations.
pub struct LlmClient {
    config: LlmConfig,
}

impl LlmClient {
    /// Create a new LLM client.
    pub fn new(config: LlmConfig) -> Self {
        Self { config }
    }

    /// Create a client with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(LlmConfig::default())
    }

    /// Call the LLM with a prompt.
    pub async fn complete(&self, system: &str, user: &str) -> Result<String> {
        let api_key = self.config.get_api_key()
            .ok_or_else(|| Error::Parse(
                "No API key found. Set OPENAI_API_KEY environment variable.".to_string()
            ))?;

        let openai_config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(&self.config.endpoint);

        let client = Client::with_config(openai_config);

        // Truncate user prompt if too long
        let truncated = if user.len() > 15000 {
            &user[..15000]
        } else {
            user
        };

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model)
            .messages([
                ChatCompletionRequestSystemMessage::from(system).into(),
                ChatCompletionRequestUserMessage::from(truncated).into(),
            ])
            .max_tokens(self.config.max_tokens as u16)
            .temperature(self.config.temperature)
            .build()
            .map_err(|e| Error::Parse(format!("Failed to build LLM request: {}", e)))?;

        let response = client.chat().create(request).await
            .map_err(|e| Error::Parse(format!("LLM API error: {}", e)))?;

        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .ok_or_else(|| Error::Parse("LLM returned no content".to_string()))?;

        Ok(content)
    }

    /// Call the LLM and parse JSON response.
    pub async fn complete_json<T: serde::de::DeserializeOwned>(
        &self,
        system: &str,
        user: &str,
    ) -> Result<T> {
        let response = self.complete(system, user).await?;
        self.parse_json(&response)
    }

    /// Parse JSON from LLM response.
    fn parse_json<T: serde::de::DeserializeOwned>(&self, text: &str) -> Result<T> {
        // Try to extract JSON from markdown code blocks
        let json_text = self.extract_json(text);

        serde_json::from_str(&json_text)
            .map_err(|e| Error::Parse(format!("Failed to parse JSON: {}. Response: {}", e, text)))
    }

    /// Extract JSON from text (handles markdown code blocks).
    fn extract_json<'a>(&self, text: &'a str) -> std::borrow::Cow<'a, str> {
        let text = text.trim();

        // Try markdown code block first
        if text.starts_with("```") {
            // Find the end of the first line (language identifier)
            if let Some(start) = text.find('\n') {
                let rest = &text[start + 1..];
                if let Some(end) = rest.find("```") {
                    return std::borrow::Cow::Borrowed(rest[..end].trim());
                }
            }
        }

        // Try to find JSON array or object
        if text.starts_with('[') || text.starts_with('{') {
            // Find matching bracket
            let open = text.chars().next().unwrap();
            let close = if open == '[' { ']' } else { '}' };

            let mut depth = 0;
            for (i, ch) in text.char_indices() {
                match ch {
                    c if c == open => depth += 1,
                    c if c == close => {
                        depth -= 1;
                        if depth == 0 {
                            return std::borrow::Cow::Borrowed(&text[..=i]);
                        }
                    }
                    _ => {}
                }
            }
        }

        std::borrow::Cow::Borrowed(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json() {
        let client = LlmClient::with_defaults();

        // Plain JSON
        let json = client.extract_json(r#"{"key": "value"}"#);
        assert_eq!(json, r#"{"key": "value"}"#);

        // JSON in code block
        let json = client.extract_json(r#"```json
{"key": "value"}
```"#);
        assert_eq!(json, r#"{"key": "value"}"#);

        // JSON array
        let json = client.extract_json(r#"[1, 2, 3]"#);
        assert_eq!(json, r#"[1, 2, 3]"#);
    }
}
