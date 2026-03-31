// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Builder pattern for creating Vectorless clients.

use std::path::PathBuf;

use crate::config::{Config, ConfigLoader};
use crate::storage::Workspace;

use super::Vectorless;

/// Builder for creating a [`Vectorless`] client.
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::client::VectorlessBuilder;
///
/// let client = VectorlessBuilder::new()
///     .with_api_key("sk-...")
///     .with_workspace("./my_workspace")
///     .build()?;
/// ```
#[derive(Debug)]
pub struct VectorlessBuilder {
    /// API key for LLM calls.
    api_key: Option<String>,

    /// Summary model name.
    summary_model: Option<String>,

    /// Retrieval model name.
    retrieval_model: Option<String>,

    /// Summary model endpoint.
    summary_endpoint: Option<String>,

    /// Retrieval model endpoint.
    retrieval_endpoint: Option<String>,

    /// Workspace path.
    workspace: Option<PathBuf>,

    /// Configuration file path.
    config_path: Option<PathBuf>,

    /// Custom configuration.
    config: Option<Config>,
}

impl VectorlessBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            api_key: None,
            summary_model: None,
            retrieval_model: None,
            summary_endpoint: None,
            retrieval_endpoint: None,
            workspace: None,
            config_path: None,
            config: None,
        }
    }

    /// Set the API key.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the summary model name.
    pub fn with_summary_model(mut self, model: impl Into<String>) -> Self {
        self.summary_model = Some(model.into());
        self
    }

    /// Set the retrieval model name.
    pub fn with_retrieval_model(mut self, model: impl Into<String>) -> Self {
        self.retrieval_model = Some(model.into());
        self
    }

    /// Set the summary model endpoint.
    pub fn with_summary_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.summary_endpoint = Some(endpoint.into());
        self
    }

    /// Set the retrieval model endpoint.
    pub fn with_retrieval_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.retrieval_endpoint = Some(endpoint.into());
        self
    }

    /// Set the workspace path.
    pub fn with_workspace(mut self, path: impl Into<PathBuf>) -> Self {
        self.workspace = Some(path.into());
        self
    }

    /// Set the configuration file path.
    pub fn with_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// Set a custom configuration.
    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    /// Build the Vectorless client.
    pub fn build(self) -> Result<Vectorless, BuildError> {
        // Load or create configuration
        let mut config = if let Some(config) = self.config {
            config
        } else if let Some(path) = self.config_path {
            ConfigLoader::new()
                .file(&path)
                .load()
                .map_err(|e| BuildError::Config(e.to_string()))?
        } else {
            Config::default()
        };

        // Apply overrides
        if let Some(key) = &self.api_key {
            config.summary.api_key = Some(key.clone());
            config.retrieval.api_key = Some(key.clone());
        }

        if let Some(model) = &self.summary_model {
            config.summary.model = model.clone();
        }

        if let Some(model) = &self.retrieval_model {
            config.retrieval.model = model.clone();
        }

        if let Some(endpoint) = &self.summary_endpoint {
            config.summary.endpoint = endpoint.clone();
        }

        if let Some(endpoint) = &self.retrieval_endpoint {
            config.retrieval.endpoint = endpoint.clone();
        }

        // Open workspace if specified
        let workspace = if let Some(path) = &self.workspace {
            Some(Workspace::open(path).map_err(|e| BuildError::Workspace(e.to_string()))?)
        } else {
            None
        };

        Ok(Vectorless {
            config,
            workspace,
            documents: std::collections::HashMap::new(),
        })
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
        assert!(builder.api_key.is_none());
        assert!(builder.workspace.is_none());
    }

    #[test]
    fn test_builder_with_options() {
        let builder = VectorlessBuilder::new()
            .with_api_key("test-key")
            .with_summary_model("gpt-4")
            .with_workspace("./test_workspace");

        assert_eq!(builder.api_key, Some("test-key".to_string()));
        assert_eq!(builder.summary_model, Some("gpt-4".to_string()));
    }
}
