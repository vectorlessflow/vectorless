// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Error types for the vectorless library.
//!
//! This module provides a comprehensive error type hierarchy for all operations.
//! All errors are consolidated into [`Error`] with specific variants for each category.

use thiserror::Error;

/// The main error type for vectorless operations.
#[derive(Debug, Error)]
pub enum Error {
    // =========================================================================
    // Document & Parsing Errors
    // =========================================================================

    /// An error occurred while parsing a document.
    #[error("Document parsing error: {0}")]
    Parse(String),

    /// Unsupported document format.
    #[error("Unsupported document format: {0}")]
    UnsupportedFormat(String),

    /// Invalid document structure.
    #[error("Invalid document structure: {0}")]
    InvalidStructure(String),

    // =========================================================================
    // Index Errors
    // =========================================================================

    /// An error occurred while building the index.
    #[error("Index building error: {0}")]
    IndexBuild(String),

    /// Index not found.
    #[error("Index not found: {0}")]
    IndexNotFound(String),

    /// Index corrupted.
    #[error("Index corrupted: {0}")]
    IndexCorrupted(String),

    // =========================================================================
    // Retrieval Errors
    // =========================================================================

    /// An error occurred during retrieval.
    #[error("Retrieval error: {0}")]
    Retrieval(String),

    /// No relevant content found.
    #[error("No relevant content found for query")]
    NoRelevantContent,

    /// Search timeout.
    #[error("Search timeout after {0}ms")]
    SearchTimeout(u64),

    // =========================================================================
    // LLM Errors
    // =========================================================================

    /// An error occurred during LLM call.
    #[error("LLM error: {0}")]
    Llm(String),

    /// LLM rate limit exceeded.
    #[error("LLM rate limit exceeded, retry after {0}ms")]
    RateLimitExceeded(u64),

    /// LLM quota exceeded.
    #[error("LLM quota exceeded")]
    QuotaExceeded,

    // =========================================================================
    // Summary Errors
    // =========================================================================

    /// An error occurred during summarization.
    #[error("Summarization error: {0}")]
    Summarization(String),

    /// Summary too long.
    #[error("Summary exceeds maximum length: {0} tokens")]
    SummaryTooLong(usize),

    // =========================================================================
    // Storage Errors
    // =========================================================================

    /// An error occurred during I/O operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Workspace error.
    #[error("Workspace error: {0}")]
    Workspace(String),

    /// Cache error.
    #[error("Cache error: {0}")]
    Cache(String),

    // =========================================================================
    // Serialization Errors
    // =========================================================================

    /// An error occurred during serialization/deserialization.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// TOML parsing error.
    #[error("TOML parsing error: {0}")]
    Toml(String),

    // =========================================================================
    // Node & Document Errors
    // =========================================================================

    /// The requested node was not found.
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// The requested document was not found.
    #[error("Document not found: {0}")]
    DocumentNotFound(String),

    // =========================================================================
    // Configuration Errors
    // =========================================================================

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    Config(String),

    /// Missing required configuration.
    #[error("Missing required configuration: {0}")]
    MissingConfig(String),

    // =========================================================================
    // Input Validation Errors
    // =========================================================================

    /// Invalid input.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Empty input.
    #[error("Empty input: {field}")]
    EmptyInput {
        /// The field that was empty.
        field: String,
    },

    /// Out of range.
    #[error("{field} out of range: expected {min}-{max}, got {actual}")]
    OutOfRange {
        /// The field that was out of range.
        field: String,
        /// Minimum allowed value.
        min: String,
        /// Maximum allowed value.
        max: String,
        /// Actual value received.
        actual: String,
    },

    // =========================================================================
    // Throttle Errors
    // =========================================================================

    /// Throttle error.
    #[error("Throttle error: {0}")]
    Throttle(String),

    /// Concurrency limit exceeded.
    #[error("Concurrency limit exceeded: {0} pending")]
    ConcurrencyLimitExceeded(usize),

    // =========================================================================
    // Timeout Errors
    // =========================================================================

    /// Operation timeout.
    #[error("Operation timeout: {0}")]
    Timeout(String),

    // =========================================================================
    // Generic Errors
    // =========================================================================

    /// A generic error with a message.
    #[error("{0}")]
    Other(String),

    /// Error with context.
    #[error("{context}: {source}")]
    WithContext {
        /// Additional context describing where/why the error occurred.
        context: String,
        /// The underlying error.
        #[source]
        source: Box<Self>,
    },
}

impl Error {
    /// Create an error with additional context.
    #[must_use]
    pub fn with_context(self, context: impl Into<String>) -> Self {
        Self::WithContext {
            context: context.into(),
            source: Box::new(self),
        }
    }

    /// Check if this is a retryable error.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimitExceeded(_)
                | Self::SearchTimeout(_)
                | Self::Timeout(_)
                | Self::Llm(_)
        )
    }

    /// Check if this is a not found error.
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::NodeNotFound(_) | Self::DocumentNotFound(_) | Self::IndexNotFound(_)
        )
    }

    /// Check if this is a timeout error.
    #[must_use]
    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout(_) | Self::SearchTimeout(_))
    }

    /// Check if this is a configuration error.
    #[must_use]
    pub fn is_config_error(&self) -> bool {
        matches!(self, Self::Config(_) | Self::MissingConfig(_))
    }

    /// Create an empty input error.
    pub fn empty_input(field: impl Into<String>) -> Self {
        Self::EmptyInput {
            field: field.into(),
        }
    }

    /// Create an out of range error.
    pub fn out_of_range(
        field: impl Into<String>,
        min: impl Into<String>,
        max: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self::OutOfRange {
            field: field.into(),
            min: min.into(),
            max: max.into(),
            actual: actual.into(),
        }
    }
}

/// A specialized result type for vectorless operations.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_context() {
        let inner = Error::Parse("test".to_string());
        let with_context = inner.with_context("While processing document");

        let msg = format!("{}", with_context);
        assert!(msg.contains("While processing document"));
        assert!(msg.contains("test"));
    }

    #[test]
    fn test_is_retryable() {
        assert!(Error::RateLimitExceeded(1000).is_retryable());
        assert!(Error::Timeout("test".to_string()).is_retryable());
        assert!(!Error::Config("test".to_string()).is_retryable());
    }

    #[test]
    fn test_is_not_found() {
        assert!(Error::NodeNotFound("1".to_string()).is_not_found());
        assert!(Error::DocumentNotFound("doc".to_string()).is_not_found());
        assert!(!Error::Parse("test".to_string()).is_not_found());
    }

    #[test]
    fn test_empty_input() {
        let err = Error::empty_input("query");
        let msg = format!("{}", err);
        assert!(msg.contains("query"));
    }

    #[test]
    fn test_out_of_range() {
        let err = Error::out_of_range("depth", "0", "10", "15");
        let msg = format!("{}", err);
        assert!(msg.contains("depth"));
        assert!(msg.contains("0"));
        assert!(msg.contains("10"));
        assert!(msg.contains("15"));
    }
}
