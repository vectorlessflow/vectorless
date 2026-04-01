// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Unified LLM error types.

use thiserror::Error;

/// LLM error types.
#[derive(Debug, Clone, Error)]
pub enum LlmError {
    /// API error from the LLM provider.
    #[error("LLM API error: {0}")]
    Api(String),

    /// Request construction error.
    #[error("Request error: {0}")]
    Request(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Response parsing error.
    #[error("Failed to parse response: {0}")]
    Parse(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Request timeout.
    #[error("Request timeout: {0}")]
    Timeout(String),

    /// No content returned.
    #[error("LLM returned no content")]
    NoContent,

    /// Retry exhausted.
    #[error("Retry exhausted after {attempts} attempts: {last_error}")]
    RetryExhausted {
        /// Number of attempts made.
        attempts: usize,
        /// The last error encountered.
        last_error: String,
    },
}

impl LlmError {
    /// Check if the error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            LlmError::Api(msg) => {
                // Rate limits and temporary failures are retryable
                let msg_lower = msg.to_lowercase();
                msg_lower.contains("rate limit") ||
                msg_lower.contains("429") ||
                msg_lower.contains("503") ||
                msg_lower.contains("502") ||
                msg_lower.contains("timeout") ||
                msg_lower.contains("overloaded")
            }
            LlmError::Timeout(_) => true,
            LlmError::RateLimit(_) => true,
            _ => false,
        }
    }

    /// Classify an API error message into the appropriate error type.
    pub fn from_api_message(msg: &str) -> Self {
        let msg_lower = msg.to_lowercase();

        if msg_lower.contains("rate limit") || msg_lower.contains("429") {
            LlmError::RateLimit(msg.to_string())
        } else if msg_lower.contains("timeout") {
            LlmError::Timeout(msg.to_string())
        } else {
            LlmError::Api(msg.to_string())
        }
    }
}

impl From<async_openai::error::OpenAIError> for LlmError {
    fn from(e: async_openai::error::OpenAIError) -> Self {
        let msg = e.to_string();
        LlmError::from_api_message(&msg)
    }
}

impl From<serde_json::Error> for LlmError {
    fn from(e: serde_json::Error) -> Self {
        LlmError::Parse(e.to_string())
    }
}

impl From<LlmError> for crate::core::Error {
    fn from(e: LlmError) -> Self {
        crate::core::Error::Llm(e.to_string())
    }
}

impl From<LlmError> for String {
    fn from(e: LlmError) -> Self {
        e.to_string()
    }
}

/// Specialized result type for LLM operations.
pub type LlmResult<T> = std::result::Result<T, LlmError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable() {
        assert!(LlmError::RateLimit("test".to_string()).is_retryable());
        assert!(LlmError::Timeout("test".to_string()).is_retryable());
        assert!(LlmError::Api("rate limit exceeded".to_string()).is_retryable());
        assert!(!LlmError::Config("test".to_string()).is_retryable());
        assert!(!LlmError::Parse("test".to_string()).is_retryable());
    }

    #[test]
    fn test_from_api_message() {
        let err = LlmError::from_api_message("Rate limit exceeded");
        assert!(matches!(err, LlmError::RateLimit(_)));

        let err = LlmError::from_api_message("Request timeout");
        assert!(matches!(err, LlmError::Timeout(_)));

        let err = LlmError::from_api_message("Internal server error");
        assert!(matches!(err, LlmError::Api(_)));
    }
}
