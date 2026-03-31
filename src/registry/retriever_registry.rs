// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retriever registry for managing retrieval strategies.
//!
//! This module provides a registry for retrievers, allowing
//! dynamic registration and retrieval of retrieval strategies.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::core::{Retriever, DocumentTree, Result, Error};
use crate::retriever::{RetrieveOptions, LlmNavigator};

/// Type alias for retriever factory functions.
type RetrieverFactory = Box<dyn Fn() -> Box<dyn Retriever> + Send + Sync>;

/// Registry for retrieval strategies.
pub struct RetrieverRegistry {
    /// Registered retriever factories by name.
    factories: Arc<RwLock<HashMap<String, RetrieverFactory>>>,
    /// Default retriever name.
    default_name: Arc<RwLock<String>>,
}

impl std::fmt::Debug for RetrieverRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let binding = self.factories.read().unwrap();
        let names: Vec<_> = binding.keys().collect();
        let default_binding = self.default_name.read().unwrap();
        f.debug_struct("RetrieverRegistry")
            .field("names", &names)
            .field("default", &*default_binding)
            .finish()
    }
}

impl RetrieverRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            factories: Arc::new(RwLock::new(HashMap::new())),
            default_name: Arc::new(RwLock::new("llm_navigate".to_string())),
        }
    }

    /// Create a registry with default retrievers.
    pub fn with_defaults() -> Self {
        let registry = Self::new();
        registry.register_defaults();
        registry
    }

    /// Register default retrievers.
    pub fn register_defaults(&self) {
        self.register("llm_navigate", || {
            Box::new(LlmNavigator::with_defaults())
        });
    }

    /// Register a retriever factory.
    pub fn register<F>(&self, name: &str, factory: F)
    where
        F: Fn() -> Box<dyn Retriever> + Send + Sync + 'static,
    {
        let mut factories = self.factories.write().unwrap();
        factories.insert(name.to_string(), Box::new(factory));
    }

    /// Get a retriever by name.
    pub fn get(&self, name: &str) -> Option<Box<dyn Retriever>> {
        let factories = self.factories.read().unwrap();
        factories.get(name).map(|f| f())
    }

    /// Get the default retriever.
    pub fn get_default(&self) -> Box<dyn Retriever> {
        let default_name = self.default_name.read().unwrap().clone();
        self.get(&default_name)
            .unwrap_or_else(|| Box::new(LlmNavigator::with_defaults()))
    }

    /// Set the default retriever name.
    pub fn set_default(&self, name: &str) {
        let mut default_name = self.default_name.write().unwrap();
        *default_name = name.to_string();
    }

    /// List registered retriever names.
    pub fn list(&self) -> Vec<String> {
        let factories = self.factories.read().unwrap();
        factories.keys().cloned().collect()
    }

    /// Check if a retriever is registered.
    pub fn has(&self, name: &str) -> bool {
        let factories = self.factories.read().unwrap();
        factories.contains_key(name)
    }

    /// Retrieve content from a document tree.
    pub async fn retrieve(
        &self,
        tree: &DocumentTree,
        query: &str,
        options: &RetrieveOptions,
    ) -> Result<Vec<String>> {
        let retriever = self.get_default();
        retriever.retrieve(tree, query, options).await
    }

    /// Retrieve content using a specific retriever.
    pub async fn retrieve_with(
        &self,
        name: &str,
        tree: &DocumentTree,
        query: &str,
        options: &RetrieveOptions,
    ) -> Result<Vec<String>> {
        let retriever = self.get(name)
            .ok_or_else(|| Error::Retrieval(format!("Retriever not found: {}", name)))?;
        retriever.retrieve(tree, query, options).await
    }
}

impl Default for RetrieverRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_defaults() {
        let registry = RetrieverRegistry::with_defaults();
        assert!(registry.has("llm_navigate"));
    }

    #[test]
    fn test_list_retrievers() {
        let registry = RetrieverRegistry::with_defaults();
        let retrievers = registry.list();
        assert!(retrievers.contains(&"llm_navigate".to_string()));
    }
}
