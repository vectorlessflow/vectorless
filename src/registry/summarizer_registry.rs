// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Summarizer registry for managing summarization strategies.
//!
//! This module provides a registry for summarizers, allowing
//! dynamic registration and retrieval of summarization strategies.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::core::{Summarizer, DocumentTree, NodeId, Result, Error};
use crate::config::SummaryConfig;

/// Type alias for summarizer factory functions.
type SummarizerFactory = Box<dyn Fn() -> Box<dyn Summarizer> + Send + Sync>;

/// Registry for summarization strategies.
pub struct SummarizerRegistry {
    /// Registered summarizer factories by name.
    factories: Arc<RwLock<HashMap<String, SummarizerFactory>>>,
    /// Default summarizer name.
    default_name: Arc<RwLock<String>>,
}

impl std::fmt::Debug for SummarizerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let factories = self.factories.read().unwrap();
        let names: Vec<_> = factories.keys().collect();
        let default = self.default_name.read().unwrap();
        f.debug_struct("SummarizerRegistry")
            .field("names", &names)
            .field("default", &*default)
            .finish()
    }
}

impl SummarizerRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            factories: Arc::new(RwLock::new(HashMap::new())),
            default_name: Arc::new(RwLock::new("llm".to_string())),
        }
    }

    /// Register a summarizer factory.
    pub fn register<F>(&self, name: &str, factory: F)
    where
        F: Fn() -> Box<dyn Summarizer> + Send + Sync + 'static,
    {
        let mut factories = self.factories.write().unwrap();
        factories.insert(name.to_string(), Box::new(factory));
    }

    /// Get a summarizer by name.
    pub fn get(&self, name: &str) -> Option<Box<dyn Summarizer>> {
        let factories = self.factories.read().unwrap();
        factories.get(name).map(|f| f())
    }

    /// Get the default summarizer.
    pub fn get_default(&self) -> Box<dyn Summarizer> {
        let default_name = self.default_name.read().unwrap().clone();
        self.get(&default_name)
            .unwrap_or_else(|| Box::new(DefaultSummarizer))
    }

    /// Set the default summarizer name.
    pub fn set_default(&self, name: &str) {
        let mut default_name = self.default_name.write().unwrap();
        *default_name = name.to_string();
    }

    /// List registered summarizer names.
    pub fn list(&self) -> Vec<String> {
        let factories = self.factories.read().unwrap();
        factories.keys().cloned().collect()
    }

    /// Check if a summarizer is registered.
    pub fn has(&self, name: &str) -> bool {
        let factories = self.factories.read().unwrap();
        factories.contains_key(name)
    }
}

impl Default for SummarizerRegistry {
    fn default() -> Self {
        let registry = Self::new();
        // Register default LLM summarizer
        registry.register("llm", || Box::new(LlmSummarizer::default()));
        registry
    }
}

/// Default summarizer that extracts first N characters.
#[derive(Debug, Clone)]
pub struct DefaultSummarizer;

#[async_trait::async_trait]
impl Summarizer for DefaultSummarizer {
    async fn summarize(&self, tree: &DocumentTree, node: NodeId) -> Result<String> {
        let node_ref = tree.get(node)
            .ok_or_else(|| Error::NodeNotFound("Node not found".to_string()))?;

        let content = &node_ref.content;
        if content.is_empty() {
            return Ok(String::new());
        }

        // Take first 200 characters as summary
        let summary: String = content.chars().take(200).collect();
        Ok(if summary.len() < content.len() {
            format!("{}...", summary)
        } else {
            summary
        })
    }
}

/// LLM-based summarizer.
#[derive(Debug, Clone)]
pub struct LlmSummarizer {
    config: SummaryConfig,
}

#[allow(dead_code)]
impl LlmSummarizer {
    /// Create a new LLM summarizer.
    pub fn new(config: SummaryConfig) -> Self {
        Self { config }
    }
}

impl Default for LlmSummarizer {
    fn default() -> Self {
        Self {
            config: SummaryConfig::default(),
        }
    }
}

#[async_trait::async_trait]
impl Summarizer for LlmSummarizer {
    async fn summarize(&self, tree: &DocumentTree, node: NodeId) -> Result<String> {
        let node_ref = tree.get(node)
            .ok_or_else(|| Error::NodeNotFound("Node not found".to_string()))?;

        if node_ref.content.is_empty() {
            return Ok(String::new());
        }

        // Use the LLM summarizer from the summarizer module
        crate::summarizer::summarize(&self.config, &node_ref.content).await
            .map_err(|e| Error::Summarization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = SummarizerRegistry::new();
        assert!(!registry.has("llm"));
    }

    #[test]
    fn test_registry_default() {
        let registry = SummarizerRegistry::default();
        assert!(registry.has("llm"));
    }
}
