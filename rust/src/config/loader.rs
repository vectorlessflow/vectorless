// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration loader.
//!
//! Loads configuration from TOML files with environment variable overrides.
//!
//! # Configuration Priority
//!
//! Configuration is loaded in this order (later overrides earlier):
//! 1. Default configuration
//! 2. Config file (if found or specified)
//! 3. Environment variables
//!
//! # Environment Variables
//!
//! | Variable | Description | Maps To |
//! |----------|-------------|---------|
//! | `OPENAI_API_KEY` | LLM API key | `llm.api_key` / `retrieval.api_key` |
//! | `VECTORLESS_MODEL` | Default LLM model | `retrieval.model` |
//! | `VECTORLESS_ENDPOINT` | LLM API endpoint | `retrieval.endpoint` |
//! | `VECTORLESS_WORKSPACE` | Workspace directory | `storage.workspace_dir` |
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::config::{ConfigLoader, Config};
//!
//! // Load from file with environment variable overrides
//! let config = ConfigLoader::new()
//!     .file("config.toml")
//!     .with_env(true)  // Enable environment variables (default: true)
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

    /// Whether to apply environment variable overrides.
    env_enabled: bool,
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
            env_enabled: true,
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

    /// Enable or disable environment variable overrides.
    ///
    /// When enabled (default), environment variables override config file values:
    /// - `OPENAI_API_KEY` → sets API key for all LLM clients
    /// - `VECTORLESS_MODEL` → sets default model
    /// - `VECTORLESS_ENDPOINT` → sets API endpoint
    /// - `VECTORLESS_WORKSPACE` → sets workspace directory
    pub fn with_env(mut self, enabled: bool) -> Self {
        self.env_enabled = enabled;
        self
    }

    /// Apply environment variable overrides to configuration.
    fn apply_env_overrides(&self, config: &mut Config) {
        if !self.env_enabled {
            return;
        }

        // OPENAI_API_KEY: Set API key for all LLM clients
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            // Set default API key
            config.llm.api_key = Some(api_key.clone());
            // Override individual client API keys if not explicitly set
            if config.llm.summary.api_key.is_none() {
                config.llm.summary.api_key = Some(api_key.clone());
            }
            if config.llm.retrieval.api_key.is_none() {
                config.llm.retrieval.api_key = Some(api_key.clone());
            }
            if config.llm.pilot.api_key.is_none() {
                config.llm.pilot.api_key = Some(api_key);
            }
        }

        // VECTORLESS_MODEL: Set default model
        if let Ok(model) = std::env::var("VECTORLESS_MODEL") {
            config.llm.summary.model = model.clone();
            config.llm.retrieval.model = model.clone();
            config.llm.pilot.model = model;
        }

        // VECTORLESS_ENDPOINT: Set API endpoint
        if let Ok(endpoint) = std::env::var("VECTORLESS_ENDPOINT") {
            config.llm.summary.endpoint = endpoint.clone();
            config.llm.retrieval.endpoint = endpoint.clone();
            config.llm.pilot.endpoint = endpoint;
        }

        // VECTORLESS_WORKSPACE: Set workspace directory
        if let Ok(workspace) = std::env::var("VECTORLESS_WORKSPACE") {
            config.storage.workspace_dir = PathBuf::from(workspace);
        }
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
        self.apply_env_overrides(&mut config);

        // Validate if requested
        if self.validate {
            let validator = self.validator.unwrap_or_default();
            validator.validate(&config)?;
        }

        Ok(config)
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
