// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration loader.
//!
//! Loads configuration from TOML files.
//!
//! # Configuration Priority
//!
//! Configuration is loaded in this order (later overrides earlier):
//! 1. Default configuration
//! 2. Config file(s)
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::config::{ConfigLoader, Config};
//!
//! // Load from file
//! let config = ConfigLoader::new()
//!     .file("config.toml")
//!     .load()?;
//!
//! // Load with validation
//! let config = ConfigLoader::new()
//!     .file("config.toml")
//!     .with_validation(true)
//!     .load()?;
//!
//! // Layered configuration
//! let config = ConfigLoader::new()
//!     .file("default.toml")
//!     .file("production.toml")
//!     .with_validation(true)
//!     .load()?;
//! # Ok::<(), vectorless::config::ConfigError>(())
//! ```

use std::path::{Path, PathBuf};
use thiserror::Error;

use super::merge::Merge;
use super::types::Config;
use super::validator::ConfigValidator;

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

    /// Configuration validation failed.
    #[error("{0}")]
    Validation(#[from] super::types::ConfigValidationError),
}

/// Configuration loader.
#[derive(Debug)]
pub struct ConfigLoader {
    /// Configuration file paths (loaded in order, later files override earlier).
    files: Vec<PathBuf>,

    /// Whether to validate after loading.
    validate: bool,

    /// Custom validator (optional).
    validator: Option<ConfigValidator>,
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigLoader {
    /// Create a new configuration loader with defaults.
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            validate: false,
            validator: None,
        }
    }

    /// Specify a configuration file to load.
    ///
    /// Multiple files can be specified; later files override earlier ones.
    pub fn file<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.files.push(path.as_ref().to_path_buf());
        self
    }

    /// Specify multiple configuration files.
    pub fn files<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        self.files
            .extend(paths.into_iter().map(|p| p.as_ref().to_path_buf()));
        self
    }

    /// Enable or disable validation after loading.
    pub fn with_validation(mut self, validate: bool) -> Self {
        self.validate = validate;
        self
    }

    /// Set a custom validator.
    pub fn with_validator(mut self, validator: ConfigValidator) -> Self {
        self.validator = Some(validator);
        self
    }

    /// Load the configuration.
    ///
    /// # Behavior
    ///
    /// 1. Start with default configuration
    /// 2. Load and merge each specified file (in order)
    /// 3. Validate configuration (if enabled)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A specified file doesn't exist
    /// - A file can't be parsed as valid TOML
    /// - Validation fails (when enabled)
    pub fn load(self) -> Result<Config, ConfigError> {
        let mut config = Config::default();

        // Load and merge each file
        for path in &self.files {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                let file_config: Config = toml::from_str(&content)?;
                config.merge(&file_config, super::merge::MergeStrategy::Replace);
            } else {
                return Err(ConfigError::NotFound(path.clone()));
            }
        }

        // Validate if requested
        if self.validate {
            let validator = self.validator.unwrap_or_default();
            validator.validate(&config)?;
        }

        Ok(config)
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
    }

    #[test]
    fn test_config_loader_defaults() {
        let config = ConfigLoader::new().load().unwrap();
        assert_eq!(config.indexer.subsection_threshold, 300);
    }

    #[test]
    fn test_config_loader_not_found() {
        let result = ConfigLoader::new().file("nonexistent_config.toml").load();

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::NotFound(_)));
    }

    #[test]
    fn test_config_loader_with_validation() {
        let config = ConfigLoader::new().with_validation(true).load().unwrap();

        assert_eq!(config.retrieval.model, "gpt-4o");
    }
}
