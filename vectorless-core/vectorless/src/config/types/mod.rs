// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration type definitions.

mod indexer;
mod llm_pool;
mod metrics;
mod retrieval;
mod storage;

use serde::{Deserialize, Serialize};

pub(crate) use indexer::IndexerConfig;
pub(crate) use llm_pool::{
    FallbackBehavior, FallbackConfig, LlmConfig, OnAllFailedBehavior, SlotConfig,
};
pub(crate) use metrics::{LlmMetricsConfig, MetricsConfig, RetrievalMetricsConfig};
pub(crate) use retrieval::RetrievalConfig;
pub(crate) use storage::{CompressionAlgorithm, StorageConfig};

/// Main configuration for vectorless.
///
/// Users typically configure via [`EngineBuilder`](crate::client::EngineBuilder):
///
/// ```rust,no_run
/// use vectorless::client::EngineBuilder;
///
/// # async fn example() -> Result<(), vectorless::BuildError> {
/// let engine = EngineBuilder::new()
///     .with_key("sk-...")
///     .with_model("gpt-4o")
///     .with_endpoint("https://api.openai.com/v1")
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// Advanced users can construct this programmatically:
///
/// ```rust,ignore
/// use vectorless::config::{Config, LlmConfig, SlotConfig};
///
/// let config = Config::new().with_llm(
///     LlmConfig::new("gpt-4o")
///         .with_api_key("sk-...")
///         .with_endpoint("https://api.openai.com/v1")
///         .with_index(SlotConfig::fast().with_model("gpt-4o-mini"))
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// LLM configuration (model, credentials, retry, throttle, fallback).
    #[serde(default)]
    pub llm: LlmConfig,

    /// Metrics configuration.
    #[serde(default)]
    pub metrics: MetricsConfig,

    /// Indexer configuration.
    #[serde(default)]
    pub indexer: IndexerConfig,

    /// Retrieval strategy configuration (search, content aggregation, etc.).
    #[serde(default)]
    pub retrieval: RetrievalConfig,

    /// Storage configuration.
    #[serde(default)]
    pub storage: StorageConfig,

    /// Document graph configuration.
    #[serde(default)]
    pub graph: crate::graph::DocumentGraphConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            llm: LlmConfig::default(),
            metrics: MetricsConfig::default(),
            indexer: IndexerConfig::default(),
            retrieval: RetrievalConfig::default(),
            storage: StorageConfig::default(),
            graph: crate::graph::DocumentGraphConfig::default(),
        }
    }
}

impl Config {
    /// Create a new configuration with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the LLM configuration.
    pub fn with_llm(mut self, llm: LlmConfig) -> Self {
        self.llm = llm;
        self
    }

    /// Set the metrics configuration.
    pub fn with_metrics(mut self, metrics: MetricsConfig) -> Self {
        self.metrics = metrics;
        self
    }

    /// Set the indexer configuration.
    pub fn with_indexer(mut self, indexer: IndexerConfig) -> Self {
        self.indexer = indexer;
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

    /// Set the document graph configuration.
    pub fn with_graph(mut self, graph: crate::graph::DocumentGraphConfig) -> Self {
        self.graph = graph;
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

        // Validate LLM slot tokens
        if self.llm.index.max_tokens == 0 {
            errors.push(ValidationError::error(
                "llm.index.max_tokens",
                "Index max tokens must be greater than 0",
            ));
        }

        if self.llm.retrieval.max_tokens == 0 {
            errors.push(ValidationError::error(
                "llm.retrieval.max_tokens",
                "Retrieval max tokens must be greater than 0",
            ));
        }

        // Validate retrieval
        if self.retrieval.top_k == 0 {
            errors.push(ValidationError::error(
                "retrieval.top_k",
                "Top K must be greater than 0",
            ));
        }

        // Validate throttle
        if self.llm.throttle.max_concurrent_requests == 0 {
            errors.push(ValidationError::error(
                "llm.throttle.max_concurrent_requests",
                "Max concurrent requests must be greater than 0",
            ));
        }

        // Validate graph
        if self.graph.min_keyword_jaccard < 0.0 || self.graph.min_keyword_jaccard > 1.0 {
            errors.push(ValidationError::error(
                "graph.min_keyword_jaccard",
                "Must be between 0.0 and 1.0",
            ));
        }
        if self.graph.max_edges_per_node == 0 {
            errors.push(ValidationError::error(
                "graph.max_edges_per_node",
                "Must be greater than 0",
            ));
        }

        // Validate fallback
        if self.llm.fallback.enabled && self.llm.fallback.models.is_empty() {
            errors.push(ValidationError::warning(
                "llm.fallback.models",
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
        assert!(config.llm.model.is_empty());
        assert!(config.llm.index.model.is_none());
        assert_eq!(config.retrieval.top_k, 3);
        assert_eq!(config.indexer.subsection_threshold, 300);
        assert!(config.metrics.enabled);
    }

    #[test]
    fn test_llm_config_defaults() {
        let config = LlmConfig::default();
        assert!(config.index.model.is_none());
        assert!(config.retrieval.model.is_none());
        assert_eq!(config.retry.max_attempts, 3);
        assert_eq!(config.throttle.max_concurrent_requests, 10);
    }

    #[test]
    fn test_config_validation_success() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_errors() {
        let mut config = Config::default();
        config.retrieval.top_k = 0;

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
