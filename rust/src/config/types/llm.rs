// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM configuration types for summary and retrieval.

use serde::{Deserialize, Serialize};

/// Generic LLM configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Model name (e.g., "gpt-4o-mini", "claude-3-haiku").
    #[serde(default = "default_model")]
    pub model: String,

    /// API endpoint.
    #[serde(default = "default_endpoint")]
    pub endpoint: String,

    /// API key.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for responses.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Temperature for generation.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_endpoint() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_max_tokens() -> usize {
    1000
}

fn default_temperature() -> f32 {
    0.0
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model: default_model(),
            endpoint: default_endpoint(),
            api_key: None,
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
        }
    }
}

impl LlmConfig {
    /// Create a new LLM config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the endpoint.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Set the API key.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the maximum tokens.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set the temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Get the API key from config.
    pub fn get_api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }
}

/// Summary model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryConfig {
    /// Model name for summarization.
    #[serde(default = "default_summary_model")]
    pub model: String,

    /// API endpoint for summary model.
    #[serde(default = "default_endpoint")]
    pub endpoint: String,

    /// API key.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for summary generation.
    #[serde(default = "default_max_summary_tokens")]
    pub max_tokens: usize,

    /// Temperature for summary generation.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_summary_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_max_summary_tokens() -> usize {
    200
}

impl Default for SummaryConfig {
    fn default() -> Self {
        Self {
            model: default_summary_model(),
            endpoint: default_endpoint(),
            api_key: None,
            max_tokens: default_max_summary_tokens(),
            temperature: default_temperature(),
        }
    }
}

impl SummaryConfig {
    /// Create a new summary config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the endpoint.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Set the API key.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the maximum tokens.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Convert to generic LLM config.
    pub fn to_llm_config(&self) -> LlmConfig {
        LlmConfig {
            model: self.model.clone(),
            endpoint: self.endpoint.clone(),
            api_key: self.api_key.clone(),
            max_tokens: self.max_tokens,
            temperature: self.temperature,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_config_defaults() {
        let config = LlmConfig::default();
        assert_eq!(config.model, "gpt-4o-mini");
        assert_eq!(config.endpoint, "https://api.openai.com/v1");
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_llm_config_builder() {
        let config = LlmConfig::new()
            .with_model("gpt-4o")
            .with_api_key("test-key")
            .with_max_tokens(2000);

        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.api_key, Some("test-key".to_string()));
        assert_eq!(config.max_tokens, 2000);
    }

    #[test]
    fn test_summary_config() {
        let config = SummaryConfig::default();
        assert_eq!(config.model, "gpt-4o-mini");
        assert_eq!(config.max_tokens, 200);
    }
}
