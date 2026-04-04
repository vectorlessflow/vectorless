// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration type definitions.
//!
//! All configuration values are defined inline in `Default` trait implementations.
//! Configuration is loaded from TOML files only - no environment variable magic.

mod content;
mod concurrency;
mod fallback;
mod indexer;
mod llm;
mod retrieval;
mod storage;

use serde::{Deserialize, Serialize};

pub use content::ContentAggregatorConfig;
pub use concurrency::ConcurrencyConfig;
pub use fallback::{FallbackBehavior, FallbackConfig, OnAllFailedBehavior};
pub use indexer::IndexerConfig;
pub use llm::{LlmConfig, SummaryConfig};
pub use retrieval::{RetrievalConfig, SearchConfig};
pub use storage::{
    CacheConfig, StorageConfig, StrategyConfig, SufficiencyConfig,
};

/// Main configuration for vectorless.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Indexer configuration.
    #[serde(default)]
    pub indexer: IndexerConfig,

    /// Summary model configuration.
    #[serde(default)]
    pub summary: SummaryConfig,

    /// Retrieval model configuration.
    #[serde(default)]
    pub retrieval: RetrievalConfig,

    /// Storage configuration.
    #[serde(default)]
    pub storage: StorageConfig,

    /// Concurrency control configuration.
    #[serde(default)]
    pub concurrency: ConcurrencyConfig,

    /// Fallback/error recovery configuration.
    #[serde(default)]
    pub fallback: FallbackConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            indexer: IndexerConfig::default(),
            summary: SummaryConfig::default(),
            retrieval: RetrievalConfig::default(),
            storage: StorageConfig::default(),
            concurrency: ConcurrencyConfig::default(),
            fallback: FallbackConfig::default(),
        }
    }
}

impl Config {
    /// Create a new configuration with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the indexer configuration.
    pub fn with_indexer(mut self, indexer: IndexerConfig) -> Self {
        self.indexer = indexer;
        self
    }

    /// Set the summary configuration.
    pub fn with_summary(mut self, summary: SummaryConfig) -> Self {
        self.summary = summary;
        self
    }

    /// Set the retrieval configuration.
    pub fn with_retrieval(mut self, retrieval: RetrievalConfig) -> Self {
        self.retrieval = retrieval;
        self
    }

    /// Set the storage configuration.
    pub fn with_storage(mut self, storage: StorageConfig) -> Self {
        self.storage = storage;
        self
    }

    /// Set the concurrency configuration.
    pub fn with_concurrency(mut self, concurrency: ConcurrencyConfig) -> Self {
        self.concurrency = concurrency;
        self
    }

    /// Set the fallback configuration.
    pub fn with_fallback(mut self, fallback: FallbackConfig) -> Self {
        self.fallback = fallback;
        self
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        let mut errors = Vec::new();

        // Validate indexer
        if self.indexer.subsection_threshold == 0 {
            errors.push(ValidationError::error(
                "indexer.subsection_threshold",
                "Subsection threshold must be greater than 0",
            ));
        }

        // Validate summary
        if self.summary.max_tokens == 0 {
            errors.push(ValidationError::error(
                "summary.max_tokens",
                "Summary max tokens must be greater than 0",
            ));
        }

        // Validate retrieval
        if self.retrieval.top_k == 0 {
            errors.push(ValidationError::error(
                "retrieval.top_k",
                "Top K must be greater than 0",
            ));
        }

        if self.retrieval.temperature < 0.0 || self.retrieval.temperature > 2.0 {
            errors.push(ValidationError::warning(
                "retrieval.temperature",
                "Temperature outside typical range [0.0, 2.0]",
            ).with_actual(self.retrieval.temperature.to_string()));
        }

        // Validate content aggregator
        if self.retrieval.content.token_budget == 0 {
            errors.push(ValidationError::error(
                "retrieval.content.token_budget",
                "Token budget must be greater than 0",
            ));
        }

        if self.retrieval.content.min_relevance_score < 0.0
            || self.retrieval.content.min_relevance_score > 1.0
        {
            errors.push(ValidationError::error(
                "retrieval.content.min_relevance_score",
                "Min relevance score must be between 0.0 and 1.0",
            )
            .with_expected("0.0 - 1.0")
            .with_actual(self.retrieval.content.min_relevance_score.to_string()));
        }

        // Validate concurrency
        if self.concurrency.max_concurrent_requests == 0 {
            errors.push(ValidationError::error(
                "concurrency.max_concurrent_requests",
                "Max concurrent requests must be greater than 0",
            ));
        }

        // Validate fallback
        if self.fallback.enabled && self.fallback.models.is_empty() {
            errors.push(ValidationError::warning(
                "fallback.models",
                "Fallback enabled but no fallback models configured",
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigValidationError { errors })
        }
    }
}

/// Configuration validation error.
#[derive(Debug, Clone, thiserror::Error)]
#[error("Configuration validation failed with {} error(s)", self.errors.len())]
pub struct ConfigValidationError {
    /// Validation errors.
    pub errors: Vec<ValidationError>,
}

/// A single validation error.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Field path (e.g., "retrieval.content.token_budget").
    pub path: String,

    /// Error message.
    pub message: String,

    /// Expected value/range.
    pub expected: Option<String>,

    /// Actual value.
    pub actual: Option<String>,

    /// Severity level.
    pub severity: Severity,
}

impl ValidationError {
    /// Create an error-level validation error.
    pub fn error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
            expected: None,
            actual: None,
            severity: Severity::Error,
        }
    }

    /// Create a warning-level validation error.
    pub fn warning(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
            expected: None,
            actual: None,
            severity: Severity::Warning,
        }
    }

    /// Create an info-level validation error.
    pub fn info(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
            expected: None,
            actual: None,
            severity: Severity::Info,
        }
    }

    /// Set the expected value.
    pub fn with_expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    /// Set the actual value.
    pub fn with_actual(mut self, actual: impl Into<String>) -> Self {
        self.actual = Some(actual.into());
        self
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let severity = match self.severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARNING",
            Severity::Info => "INFO",
        };
        write!(f, "[{}] {}: {}", severity, self.path, self.message)?;
        if let Some(ref expected) = self.expected {
            write!(f, " (expected: {})", expected)?;
        }
        if let Some(ref actual) = self.actual {
            write!(f, " (actual: {})", actual)?;
        }
        Ok(())
    }
}

/// Validation severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Error - must fix.
    Error,
    /// Warning - should fix.
    Warning,
    /// Info - suggestion.
    Info,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert_eq!(config.indexer.subsection_threshold, 300);
        assert_eq!(config.summary.model, "gpt-4o-mini");
        assert_eq!(config.retrieval.model, "gpt-4o");
        assert_eq!(config.concurrency.max_concurrent_requests, 10);
    }

    #[test]
    fn test_config_validation_success() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_errors() {
        let mut config = Config::default();
        config.retrieval.content.token_budget = 0;
        config.retrieval.content.min_relevance_score = 1.5;

        let result = config.validate();
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(!err.errors.is_empty());
    }

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError::error("test.field", "Invalid value")
            .with_expected(">= 1")
            .with_actual("0");

        let display = format!("{}", err);
        assert!(display.contains("ERROR"));
        assert!(display.contains("test.field"));
        assert!(display.contains("expected"));
    }
}
