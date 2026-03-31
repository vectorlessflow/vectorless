// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration loader.
//!
//! Supports loading configuration from:
//! - TOML files
//! - Environment variables
//! - Programmatic overrides

use super::types::Config;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Configuration loading errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Failed to read configuration file.
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse TOML.
    #[error("Failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),

    /// Configuration file not found.
    #[error("Config file not found: {0}")]
    NotFound(PathBuf),

    /// Invalid configuration value.
    #[error("Invalid configuration: {0}")]
    Invalid(String),
}

/// Configuration loader with layered overrides.
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::config::{ConfigLoader, Config};
///
/// // Load from file
/// let config = ConfigLoader::new()
///     .file("config.toml")
///     .env_prefix("VECTORLESS")
///     .load()?;
///
/// // Or use defaults
/// let config = Config::default();
/// ```
#[derive(Debug, Default)]
pub struct ConfigLoader {
    /// Configuration file path.
    file: Option<PathBuf>,

    /// Environment variable prefix.
    env_prefix: Option<String>,

    /// Programmatic overrides.
    overrides: Vec<ConfigOverride>,
}

/// A single configuration override.
#[derive(Debug)]
enum ConfigOverride {
    /// Override a specific field by path.
    Field { path: String, value: String },
}

impl ConfigLoader {
    /// Create a new configuration loader with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Specify a configuration file to load.
    pub fn file<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.file = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set environment variable prefix for overrides.
    ///
    /// For example, with prefix `VECTORLESS`, the loader will check
    /// `VECTORLESS_SUMMARY_MODEL`, `VECTORLESS_RETRIEVAL_MODEL`, etc.
    pub fn env_prefix(mut self, prefix: &str) -> Self {
        self.env_prefix = Some(prefix.to_string());
        self
    }

    /// Load the configuration.
    ///
    /// This method:
    /// 1. Starts with default configuration
    /// 2. Loads from file if specified
    /// 3. Applies environment variable overrides
    /// 4. Applies programmatic overrides
    pub fn load(self) -> Result<Config, ConfigError> {
        let mut config = Config::default();

        // Load from file if specified
        if let Some(ref path) = self.file {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                let file_config: Config = toml::from_str(&content)?;
                config = merge_config(config, file_config);
            } else {
                return Err(ConfigError::NotFound(path.clone()));
            }
        }

        // Apply environment variable overrides
        if let Some(ref prefix) = self.env_prefix {
            config = apply_env_overrides(config, prefix);
        }

        Ok(config)
    }
}

/// Merge two configurations, with `other` taking precedence.
fn merge_config(base: Config, other: Config) -> Config {
    Config {
        indexer: IndexerConfig {
            subsection_threshold: if other.indexer.subsection_threshold != default_subsection_threshold() {
                other.indexer.subsection_threshold
            } else {
                base.indexer.subsection_threshold
            },
            max_segment_tokens: if other.indexer.max_segment_tokens != default_max_segment_tokens() {
                other.indexer.max_segment_tokens
            } else {
                base.indexer.max_segment_tokens
            },
            max_summary_tokens: if other.indexer.max_summary_tokens != default_max_summary_tokens() {
                other.indexer.max_summary_tokens
            } else {
                base.indexer.max_summary_tokens
            },
        },
        summary: SummaryConfig {
            model: if other.summary.model != default_summary_model() {
                other.summary.model
            } else {
                base.summary.model
            },
            endpoint: if other.summary.endpoint != default_summary_endpoint() {
                other.summary.endpoint
            } else {
                base.summary.endpoint
            },
            api_key: other.summary.api_key.or(base.summary.api_key),
            max_tokens: if other.summary.max_tokens != default_summary_max_tokens() {
                other.summary.max_tokens
            } else {
                base.summary.max_tokens
            },
            temperature: if other.summary.temperature != default_temperature() {
                other.summary.temperature
            } else {
                base.summary.temperature
            },
        },
        retrieval: RetrievalConfig {
            model: if other.retrieval.model != default_retrieval_model() {
                other.retrieval.model
            } else {
                base.retrieval.model
            },
            endpoint: if other.retrieval.endpoint != default_retrieval_endpoint() {
                other.retrieval.endpoint
            } else {
                base.retrieval.endpoint
            },
            api_key: other.retrieval.api_key.or(base.retrieval.api_key),
            max_tokens: if other.retrieval.max_tokens != default_retrieval_max_tokens() {
                other.retrieval.max_tokens
            } else {
                base.retrieval.max_tokens
            },
            temperature: if other.retrieval.temperature != default_temperature() {
                other.retrieval.temperature
            } else {
                base.retrieval.temperature
            },
            top_k: if other.retrieval.top_k != default_top_k() {
                other.retrieval.top_k
            } else {
                base.retrieval.top_k
            },
        },
        storage: StorageConfig {
            data_dir: if other.storage.data_dir != default_data_dir() {
                other.storage.data_dir
            } else {
                base.storage.data_dir
            },
            index_dir: if other.storage.index_dir != default_index_dir() {
                other.storage.index_dir
            } else {
                base.storage.index_dir
            },
        },
    }
}

use super::types::{IndexerConfig, SummaryConfig, RetrievalConfig, StorageConfig};
use super::types::{
    default_subsection_threshold, default_max_segment_tokens, default_max_summary_tokens,
    default_summary_model, default_summary_endpoint, default_summary_max_tokens, default_temperature,
    default_retrieval_model, default_retrieval_endpoint, default_retrieval_max_tokens, default_top_k,
    default_data_dir, default_index_dir,
};

/// Apply environment variable overrides to the configuration.
fn apply_env_overrides(mut config: Config, prefix: &str) -> Config {
    // Summary model overrides
    if let Ok(model) = std::env::var(format!("{}_SUMMARY_MODEL", prefix)) {
        config.summary.model = model;
    }
    if let Ok(endpoint) = std::env::var(format!("{}_SUMMARY_ENDPOINT", prefix)) {
        config.summary.endpoint = endpoint;
    }
    if let Ok(api_key) = std::env::var(format!("{}_SUMMARY_API_KEY", prefix)) {
        config.summary.api_key = Some(api_key);
    }

    // Retrieval model overrides
    if let Ok(model) = std::env::var(format!("{}_RETRIEVAL_MODEL", prefix)) {
        config.retrieval.model = model;
    }
    if let Ok(endpoint) = std::env::var(format!("{}_RETRIEVAL_ENDPOINT", prefix)) {
        config.retrieval.endpoint = endpoint;
    }
    if let Ok(api_key) = std::env::var(format!("{}_RETRIEVAL_API_KEY", prefix)) {
        config.retrieval.api_key = Some(api_key);
    }

    // Storage overrides
    if let Ok(data_dir) = std::env::var(format!("{}_DATA_DIR", prefix)) {
        config.storage.data_dir = PathBuf::from(data_dir);
    }
    if let Ok(index_dir) = std::env::var(format!("{}_INDEX_DIR", prefix)) {
        config.storage.index_dir = PathBuf::from(index_dir);
    }

    // Common API key fallback
    if let Ok(api_key) = std::env::var(format!("{}_API_KEY", prefix)) {
        if config.summary.api_key.is_none() {
            config.summary.api_key = Some(api_key.clone());
        }
        if config.retrieval.api_key.is_none() {
            config.retrieval.api_key = Some(api_key);
        }
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.indexer.subsection_threshold, 300);
        assert_eq!(config.summary.model, "glm-5-flash");
        assert_eq!(config.retrieval.model, "glm-5");
    }

    #[test]
    fn test_config_loader_defaults() {
        let config = ConfigLoader::new().load().unwrap();
        assert_eq!(config.indexer.subsection_threshold, 300);
    }
}
