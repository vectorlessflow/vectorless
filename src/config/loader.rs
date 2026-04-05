// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration loader.
//!
//! Loads configuration from TOML files with optional environment variable
//! overrides and validation.
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
//! // Load with environment variable override
//! let config = ConfigLoader::new()
//!     .file("config.toml")
//!     .with_env("VECTORLESS_")
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

    /// Environment variable error.
    #[error("Environment variable error: {0}")]
    Env(String),
}

/// Configuration loader.
#[derive(Debug)]
pub struct ConfigLoader {
    /// Configuration file paths (loaded in order, later files override earlier).
    files: Vec<PathBuf>,

    /// Environment variable prefix (optional).
    env_prefix: Option<String>,

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
            env_prefix: None,
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

    /// Enable environment variable override.
    ///
    /// Variables like `VECTORLESS_SUMMARY__API_KEY` override config values.
    /// Use `__` (double underscore) to separate nested keys.
    pub fn with_env(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = Some(prefix.into());
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
    /// 3. Apply environment variable overrides (if enabled)
    /// 4. Validate configuration (if enabled)
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

        // Apply environment variable overrides
        if let Some(ref prefix) = self.env_prefix {
            self.apply_env_overrides(&mut config, prefix)?;
        }

        // Validate if requested
        if self.validate {
            let validator = self.validator.unwrap_or_default();
            validator.validate(&config)?;
        }

        Ok(config)
    }

    /// Apply environment variable overrides to the configuration.
    fn apply_env_overrides(&self, config: &mut Config, prefix: &str) -> Result<(), ConfigError> {
        for (key, value) in std::env::vars() {
            if !key.starts_with(prefix) {
                continue;
            }

            // Parse the path: VECTORLESS_SUMMARY__API_KEY -> ["summary", "api_key"]
            let path_str = key.trim_start_matches(prefix).trim_start_matches('_');
            let parts: Vec<&str> = path_str.split("__").collect();

            if parts.is_empty() {
                continue;
            }

            // Apply the override
            self.set_by_path(config, &parts, &value)?;
        }

        Ok(())
    }

    /// Set a configuration value by path.
    fn set_by_path(
        &self,
        config: &mut Config,
        path: &[&str],
        value: &str,
    ) -> Result<(), ConfigError> {
        match path {
            ["summary", "api_key"] => {
                config.summary.api_key = Some(value.to_string());
            }
            ["summary", "model"] => {
                config.summary.model = value.to_string();
            }
            ["summary", "endpoint"] => {
                config.summary.endpoint = value.to_string();
            }
            ["summary", "max_tokens"] => {
                config.summary.max_tokens = value
                    .parse()
                    .map_err(|e| ConfigError::Env(format!("Invalid max_tokens: {}", e)))?;
            }
            ["retrieval", "api_key"] => {
                config.retrieval.api_key = Some(value.to_string());
            }
            ["retrieval", "model"] => {
                config.retrieval.model = value.to_string();
            }
            ["retrieval", "endpoint"] => {
                config.retrieval.endpoint = value.to_string();
            }
            ["retrieval", "top_k"] => {
                config.retrieval.top_k = value
                    .parse()
                    .map_err(|e| ConfigError::Env(format!("Invalid top_k: {}", e)))?;
            }
            ["storage", "workspace_dir"] => {
                config.storage.workspace_dir = PathBuf::from(value);
            }
            ["concurrency", "max_concurrent_requests"] => {
                config.concurrency.max_concurrent_requests = value.parse().map_err(|e| {
                    ConfigError::Env(format!("Invalid max_concurrent_requests: {}", e))
                })?;
            }
            _ => {
                // Unknown path - could log a warning
            }
        }

        Ok(())
    }
}

/// Default configuration file names to search for.
pub const CONFIG_FILE_NAMES: &[&str] = &["vectorless.toml", "config.toml", ".vectorless.toml"];

/// Find a configuration file in current or parent directories.
pub fn find_config_file() -> Option<PathBuf> {
    let current_dir = std::env::current_dir().ok()?;

    // Search in current directory first
    for name in CONFIG_FILE_NAMES {
        let path = current_dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }

    // Search in parent directories (up to 3 levels)
    let mut dir = current_dir.as_path();
    for _ in 0..3 {
        if let Some(parent) = dir.parent() {
            for name in CONFIG_FILE_NAMES {
                let path = parent.join(name);
                if path.exists() {
                    return Some(path);
                }
            }
            dir = parent;
        } else {
            break;
        }
    }

    None
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
