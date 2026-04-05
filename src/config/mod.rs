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
//!     .file("config.toml")
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
//!     .with_env("VECTORLESS_")     // Environment overrides
//!     .with_validation(true)
//!     .load()?;
//! # Ok::<(), vectorless::config::ConfigError>(())
//! ```
//!
//! # Environment Variables
//!
//! When enabled with `with_env()`, environment variables can override config:
//!
//! | Variable | Config Path |
//! |----------|-------------|
//! | `VECTORLESS_SUMMARY__API_KEY` | `summary.api_key` |
//! | `VECTORLESS_RETRIEVAL__TOP_K` | `retrieval.top_k` |
//! | `VECTORLESS_STORAGE__WORKSPACE_DIR` | `storage.workspace_dir` |
//!
//! # Configuration Sections
//!
//! - `[indexer]` — Document indexing parameters
//! - `[summary]` — Summarization model settings
//! - `[retrieval]` — Retrieval model settings
//! - `[retrieval.search]` — Search algorithm configuration
//! - `[retrieval.sufficiency]` — Sufficiency checker settings
//! - `[retrieval.content]` — Content aggregator settings
//! - `[retrieval.strategy]` — Strategy-specific settings
//! - `[retrieval.cache]` — Cache configuration
//! - `[storage]` — Storage paths
//! - `[concurrency]` — Concurrency control
//! - `[fallback]` — Error recovery settings

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
    LlmConfig,
    OnAllFailedBehavior,
    // Retrieval configs
    RetrievalConfig,
    SearchConfig,
    Severity,
    // Storage and sufficiency
    StorageConfig,
    StrategyConfig,
    SufficiencyConfig,
    SummaryConfig,
    ValidationError,
};
pub use validator::{ConfigValidator, ValidationRule};
