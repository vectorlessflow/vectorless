// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Builder pattern for creating Vectorless clients.

use std::path::PathBuf;

use crate::config::{Config, ConfigLoader};
use crate::storage::Workspace;

use super::Vectorless;

/// Default configuration file names to search for.
const CONFIG_FILE_NAMES: &[&str] = &["vectorless.toml", "config.toml", ".vectorless.toml"];

/// Builder for creating a [`Vectorless`] client.
///
/// The builder uses sensible defaults and automatically loads
/// LLM configuration from environment variables or config files.
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::client::VectorlessBuilder;
///
/// let client = VectorlessBuilder::new()
///     .with_workspace("./my_workspace")
///     .build()?;
/// # Ok::<(), vectorless::core::Error>(())
/// ```
#[derive(Debug)]
pub struct VectorlessBuilder {
    /// Workspace path.
    workspace: Option<PathBuf>,

    /// Configuration file path.
    config_path: Option<PathBuf>,

    /// Custom configuration.
    config: Option<Config>,
}

impl VectorlessBuilder {
    /// Create a new builder with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            workspace: None,
            config_path: None,
            config: None,
        }
    }

    /// Set the workspace path for document persistence.
    #[must_use]
    pub fn with_workspace(mut self, path: impl Into<PathBuf>) -> Self {
        self.workspace = Some(path.into());
        self
    }

    /// Set the configuration file path.
    #[must_use]
    pub fn with_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// Set a custom configuration.
    #[must_use]
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    /// Search for config file in current directory and parent directories.
    fn find_config_file() -> Option<PathBuf> {
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

    /// Build the Vectorless client.
    ///
    /// Configuration is loaded in this order (later overrides earlier):
    /// 1. Default configuration
    /// 2. Configuration file (auto-detected or specified)
    /// 3. Environment variables (VECTORLESS_* or standard LLM vars)
    ///
    /// # Errors
    ///
    /// Returns a [`BuildError`] if:
    /// - Configuration loading fails
    /// - Workspace creation fails
    pub fn build(self) -> Result<Vectorless, BuildError> {
        // Load or create configuration
        let config = if let Some(config) = self.config {
            // Use explicitly provided config
            config
        } else if let Some(path) = self.config_path {
            // Load from specified path
            ConfigLoader::new()
                .file(&path)
                .env_prefix("VECTORLESS")
                .load()
                .map_err(|e| BuildError::Config(e.to_string()))?
        } else if let Some(config_path) = Self::find_config_file() {
            // Auto-detect config file
            ConfigLoader::new()
                .file(&config_path)
                .env_prefix("VECTORLESS")
                .load()
                .map_err(|e| BuildError::Config(format!("Failed to load {}: {}", config_path.display(), e)))?
        } else {
            // Use defaults with environment variable overrides
            ConfigLoader::new()
                .env_prefix("VECTORLESS")
                .load()
                .map_err(|e| BuildError::Config(e.to_string()))?
        };

        // Open workspace: prefer explicit path, fallback to config
        let workspace = if let Some(path) = &self.workspace {
            Some(Workspace::open(path).map_err(|e| BuildError::Workspace(e.to_string()))?)
        } else {
            // Use workspace_dir from config
            Some(Workspace::open(&config.storage.workspace_dir)
                .map_err(|e| BuildError::Workspace(e.to_string()))?)
        };

        // Create pipeline executor with LLM client if API key is available
        let executor = if let Some(api_key) = config.summary.api_key.clone() {
            // Create LlmConfig from SummaryConfig
            let llm_config = crate::llm::LlmConfig::new(&config.summary.model)
                .with_endpoint(config.summary.endpoint.clone())
                .with_api_key(api_key)
                .with_max_tokens(config.summary.max_tokens)
                .with_temperature(config.summary.temperature);

            let llm_client = crate::llm::LlmClient::new(llm_config);
            crate::core::index::PipelineExecutor::with_llm(llm_client)
        } else {
            crate::core::index::PipelineExecutor::new()
        };

        // Create adaptive retriever
        let retriever = crate::core::retriever::AdaptiveRetriever::new();

        Ok(Vectorless::with_components(config, workspace, retriever, executor))
    }
}

impl Default for VectorlessBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Error during client build.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Workspace error.
    #[error("Workspace error: {0}")]
    Workspace(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let builder = VectorlessBuilder::new();
        assert!(builder.workspace.is_none());
    }

    #[test]
    fn test_builder_with_workspace() {
        let builder = VectorlessBuilder::new()
            .with_workspace("./test_workspace");

        assert_eq!(builder.workspace, Some(PathBuf::from("./test_workspace")));
    }
}
