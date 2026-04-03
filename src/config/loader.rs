// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration loader.
//!
//! Loads configuration from TOML files only.
//! All configuration comes from config files, not environment variables.
//! This ensures configuration is explicit and traceable.

use std::path::{Path, PathBuf};
use thiserror::Error;

use super::types::Config;

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

/// Configuration loader.
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::config::{ConfigLoader, Config};
///
/// // Load from file
/// let config = ConfigLoader::new()
///     .file("config.toml")
///     .load()?;
///
/// // Or use defaults
/// let config = Config::default();
/// # Ok::<(), vectorless::config::ConfigError>(())
/// ```
#[derive(Debug, Default)]
pub struct ConfigLoader {
    /// Configuration file path.
    file: Option<PathBuf>,
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

    /// Load the configuration.
    ///
    /// If no file is specified, returns default configuration.
    /// If file is specified but doesn't exist, returns an error.
    pub fn load(self) -> Result<Config, ConfigError> {
        if let Some(ref path) = self.file {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                let config: Config = toml::from_str(&content)?;
                Ok(config)
            } else {
                Err(ConfigError::NotFound(path.clone()))
            }
        } else {
            Ok(Config::default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.indexer.subsection_threshold, 300);
        assert_eq!(config.summary.model, "gpt-4o-mini");
        assert_eq!(config.retrieval.model, "gpt-4o");
        assert_eq!(config.concurrency.max_concurrent_requests, 10);
        assert_eq!(config.concurrency.requests_per_minute, 500);
        assert!(config.concurrency.enabled);
        assert!(config.concurrency.semaphore_enabled);
    }

    #[test]
    fn test_config_loader_defaults() {
        let config = ConfigLoader::new().load().unwrap();
        assert_eq!(config.indexer.subsection_threshold, 300);
    }
}
