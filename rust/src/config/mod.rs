// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration management for vectorless.
//!
//! This module provides comprehensive configuration loading, validation,
//! and management:
//!
//! - [`Config`] — Main configuration structure
//! - [`ConfigLoader`] — Load configuration from TOML files
//! - [`ConfigValidator`] — Validate configuration values
//! - [`ConfigDocs`] — Generate configuration documentation
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use vectorless::config::{Config, ConfigLoader};
//!
//! // Load from file
//! let config = ConfigLoader::new()
//!     .file("vectorless.toml")
//!     .with_validation(true)
//!     .load()?;
//!
//! // Or use defaults
//! let config = Config::default();
//! # Ok::<(), vectorless::config::ConfigError>(())
//! ```
//!
//! # Layered Configuration
//!
//! Multiple configuration files can be layered:
//!
//! ```rust,no_run
//! use vectorless::config::ConfigLoader;
//!
//! let config = ConfigLoader::new()
//!     .file("default.toml")        // Base defaults
//!     .file("production.toml")     // Production overrides
//!     .with_validation(true)
//!     .load()?;
//! # Ok::<(), vectorless::config::ConfigError>(())
//! ```
//!
//! # Configuration Sections
//!
//! - `[llm]` — Unified LLM configuration (pool, retry, throttle, fallback)
//! - `[metrics]` — Unified metrics configuration
//! - `[pilot]` — Pilot navigation configuration
//! - `[indexer]` — Document indexing parameters
//! - `[retrieval]` — Retrieval model settings
//! - `[storage]` — Storage paths

mod docs;
mod loader;
mod merge;
mod types;
mod validator;

// Re-export main types
pub use docs::ConfigDocs;
pub use loader::{CONFIG_FILE_NAMES, ConfigError, ConfigLoader, find_config_file};
pub use merge::{ConfigOverlay, Merge, MergeStrategy};
pub use types::{
    CacheConfig,
    CompressionAlgorithm,
    CompressionConfig,
    // Concurrency
    ConcurrencyConfig,
    // Main config
    Config,
    // Validation
    ConfigValidationError,
    // Content aggregator
    ContentAggregatorConfig,
    // Fallback
    FallbackBehavior,
    FallbackConfig,
    // Indexer
    IndexerConfig,
    // LLM configs
    LlmClientConfig,
    LlmConfig,
    LlmFallbackBehavior,
    LlmFallbackConfig,
    LlmMetricsConfig,
    LlmOnAllFailedBehavior,
    LlmPoolConfig,
    MetricsConfig,
    OnAllFailedBehavior,
    PilotMetricsConfig,
    // Retrieval configs
    RetrievalConfig,
    RetrievalMetricsConfig,
    RetryConfig,
    SearchConfig,
    Severity,
    // Storage and sufficiency
    StorageConfig,
    StrategyConfig,
    SufficiencyConfig,
    SummaryConfig,
    ThrottleConfig,
    ValidationError,
};
pub use validator::{ConfigValidator, ValidationRule};
