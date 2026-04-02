// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retriever registry for managing retrieval strategies.
//!
//! This module provides a registry for retrievers, allowing
//! dynamic registration and retrieval of retrieval strategies.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::core::{Retriever, VectorlessTree, Result, Error};
use crate::core::retriever::{RetrieveOptions, RetrieveResponse};
use crate::retriever::LlmNavigator;
use crate::config::RetrieverType;

/// Type alias for retriever factory functions.
type RetrieverFactory = Box<dyn Fn() -> Box<dyn Retriever> + Send + Sync>;

/// Registry for retrieval strategies.
pub struct RetrieverRegistry {
    /// Registered retriever factories by type.
    factories: Arc<RwLock<HashMap<RetrieverType, RetrieverFactory>>>,
    /// Default retriever type.
    default_type: Arc<RwLock<RetrieverType>>,
}

impl std::fmt::Debug for RetrieverRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let binding = self.factories.read().unwrap();
        let types: Vec<_> = binding.keys().collect();
        let default_binding = self.default_type.read().unwrap();
        f.debug_struct("RetrieverRegistry")
            .field("types", &types)
            .field("default", &*default_binding)
            .finish()
    }
}

impl RetrieverRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            factories: Arc::new(RwLock::new(HashMap::new())),
            default_type: Arc::new(RwLock::new(RetrieverType::default())),
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
        self.register(RetrieverType::LlmNavigate, || {
            Box::new(LlmNavigator::with_defaults())
        });
    }

    /// Register a retriever factory by type.
    pub fn register<F>(&self, retriever_type: RetrieverType, factory: F)
    where
        F: Fn() -> Box<dyn Retriever> + Send + Sync + 'static,
    {
        let mut factories = self.factories.write().unwrap();
        factories.insert(retriever_type, Box::new(factory));
    }

    /// Get a retriever by type.
    pub fn get(&self, retriever_type: RetrieverType) -> Option<Box<dyn Retriever>> {
        let factories = self.factories.read().unwrap();
        factories.get(&retriever_type).map(|f| f())
    }

    /// Get the default retriever.
    pub fn get_default(&self) -> Box<dyn Retriever> {
        let default_type = *self.default_type.read().unwrap();
        self.get(default_type)
            .unwrap_or_else(|| Box::new(LlmNavigator::with_defaults()))
    }

    /// Set the default retriever type.
    pub fn set_default(&self, retriever_type: RetrieverType) {
        let mut default_type = self.default_type.write().unwrap();
        *default_type = retriever_type;
    }

    /// List registered retriever types.
    pub fn list(&self) -> Vec<RetrieverType> {
        let factories = self.factories.read().unwrap();
        factories.keys().copied().collect()
    }

    /// Check if a retriever is registered.
    pub fn has(&self, retriever_type: RetrieverType) -> bool {
        let factories = self.factories.read().unwrap();
        factories.contains_key(&retriever_type)
    }

    /// Retrieve content from a document tree.
    pub async fn retrieve(
        &self,
        tree: &VectorlessTree,
        query: &str,
        options: &RetrieveOptions,
    ) -> Result<RetrieveResponse> {
        let retriever = self.get_default();
        retriever.retrieve(tree, query, options).await
            .map_err(|e| Error::Retrieval(e.to_string()))
    }

    /// Retrieve content using a specific retriever type.
    pub async fn retrieve_with(
        &self,
        retriever_type: RetrieverType,
        tree: &VectorlessTree,
        query: &str,
        options: &RetrieveOptions,
    ) -> Result<RetrieveResponse> {
        let retriever = self.get(retriever_type)
            .ok_or_else(|| Error::Retrieval(format!("Retriever not found: {:?}", retriever_type)))?;
        retriever.retrieve(tree, query, options).await
            .map_err(|e| Error::Retrieval(e.to_string()))
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
        assert!(registry.has(RetrieverType::LlmNavigate));
    }

    #[test]
    fn test_list_retrievers() {
        let registry = RetrieverRegistry::with_defaults();
        let retrievers = registry.list();
        assert!(retrievers.contains(&RetrieverType::LlmNavigate));
    }
}
