// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Lazy summary strategy - generate summaries on-demand at query time.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::llm::LlmClient;

use super::{SummaryGenerator, SummaryStrategyConfig};

/// Lazy summary strategy - generates summaries on-demand.
///
/// Summaries are generated when first requested and optionally cached
/// for future use.
pub struct LazyStrategy {
    /// Summary generator.
    generator: Arc<RwLock<Box<dyn SummaryGenerator>>>,
    /// Cache of generated summaries (node_id -> summary).
    cache: Arc<RwLock<HashMap<String, String>>>,
    /// Whether to persist generated summaries.
    persist: bool,
    /// Configuration.
    config: SummaryStrategyConfig,
}

impl LazyStrategy {
    /// Create a new lazy strategy with LLM client.
    pub fn new(client: LlmClient) -> Self {
        Self {
            generator: Arc::new(RwLock::new(Box::new(super::LlmSummaryGenerator::new(
                client,
            )))),
            cache: Arc::new(RwLock::new(HashMap::new())),
            persist: false,
            config: SummaryStrategyConfig::default(),
        }
    }

    /// Create with persistence enabled.
    pub fn with_persist(client: LlmClient, persist: bool) -> Self {
        Self {
            generator: Arc::new(RwLock::new(Box::new(super::LlmSummaryGenerator::new(
                client,
            )))),
            cache: Arc::new(RwLock::new(HashMap::new())),
            persist,
            config: SummaryStrategyConfig::default(),
        }
    }

    /// Create with custom generator.
    pub fn with_generator(generator: Box<dyn SummaryGenerator>) -> Self {
        Self {
            generator: Arc::new(RwLock::new(generator)),
            cache: Arc::new(RwLock::new(HashMap::new())),
            persist: false,
            config: SummaryStrategyConfig::default(),
        }
    }

    /// Set persistence mode.
    pub fn with_persist_mode(mut self, persist: bool) -> Self {
        self.persist = persist;
        self
    }

    /// Set configuration.
    pub fn with_config(mut self, config: SummaryStrategyConfig) -> Self {
        self.config = config;
        self
    }

    /// Check if a cached summary exists.
    pub async fn has_cached(&self, node_id: &str) -> bool {
        let cache = self.cache.read().await;
        cache.contains_key(node_id)
    }

    /// Get a cached summary if available.
    pub async fn get_cached(&self, node_id: &str) -> Option<String> {
        let cache = self.cache.read().await;
        cache.get(node_id).cloned()
    }

    /// Get or generate a summary.
    ///
    /// Returns the cached summary if available, otherwise generates a new one.
    pub async fn get_or_generate(
        &self,
        node_id: &str,
        title: &str,
        content: &str,
    ) -> crate::llm::LlmResult<String> {
        // Check cache first
        if self.persist {
            if let Some(cached) = self.get_cached(node_id).await {
                return Ok(cached);
            }
        }

        // Generate new summary
        let generator = self.generator.read().await;
        let summary = generator.generate(title, content).await?;

        // Cache if persistence is enabled
        if self.persist {
            let mut cache = self.cache.write().await;
            cache.insert(node_id.to_string(), summary.clone());
        }

        Ok(summary)
    }

    /// Pre-populate the cache with existing summaries.
    pub async fn populate_cache(&self, summaries: HashMap<String, String>) {
        let mut cache = self.cache.write().await;
        cache.extend(summaries);
    }

    /// Clear the cache.
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Get cache size.
    pub async fn cache_size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }

    /// Check if persistence is enabled.
    pub fn is_persist_enabled(&self) -> bool {
        self.persist
    }

    /// Get the configuration.
    pub fn config(&self) -> &SummaryStrategyConfig {
        &self.config
    }
}

impl std::fmt::Debug for LazyStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LazyStrategy")
            .field("persist", &self.persist)
            .field("config", &self.config)
            .finish()
    }
}
