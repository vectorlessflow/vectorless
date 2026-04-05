// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Core Retriever trait and related types.

use async_trait::async_trait;

use super::types::{RetrieveOptions, RetrieveResponse};
use crate::document::DocumentTree;

/// Result type for retriever operations.
pub type RetrieverResult<T> = Result<T, RetrieverError>;

/// Errors that can occur during retrieval.
#[derive(Debug, thiserror::Error)]
pub enum RetrieverError {
    /// The document tree is empty or invalid.
    #[error("Invalid document tree: {0}")]
    InvalidTree(String),

    /// No relevant nodes found for the query.
    #[error("No relevant nodes found for query")]
    NoResults,

    /// LLM call failed during retrieval.
    #[error("LLM error: {0}")]
    LlmError(String),

    /// Embedding generation failed.
    #[error("Embedding error: {0}")]
    EmbeddingError(String),

    /// Cache operation failed.
    #[error("Cache error: {0}")]
    CacheError(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Internal error during retrieval.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Trait for document retrieval strategies.
///
/// Implementations provide different approaches to navigating
/// the document tree and finding relevant content.
#[async_trait]
pub trait Retriever: Send + Sync {
    /// Retrieve relevant content for the given query.
    ///
    /// # Arguments
    ///
    /// * `tree` - The document tree to search
    /// * `query` - The user's query string
    /// * `options` - Retrieval options controlling behavior
    ///
    /// # Returns
    ///
    /// A `RetrieveResponse` containing the retrieved content and metadata.
    async fn retrieve(
        &self,
        tree: &DocumentTree,
        query: &str,
        options: &RetrieveOptions,
    ) -> RetrieverResult<RetrieveResponse>;

    /// Get the name of this retriever for logging/debugging.
    fn name(&self) -> &str;

    /// Check if this retriever supports the given options.
    ///
    /// Some retrievers may not support all features (e.g., sufficiency checking).
    fn supports_options(&self, _options: &RetrieveOptions) -> bool {
        true
    }

    /// Estimate the cost of a retrieval operation.
    ///
    /// Returns an estimated number of LLM calls or tokens that will be used.
    /// Useful for cost-aware strategy selection.
    fn estimate_cost(&self, tree: &DocumentTree, options: &RetrieveOptions) -> CostEstimate {
        let node_count = tree.node_count();
        CostEstimate {
            llm_calls: node_count / 2, // Rough estimate
            tokens: node_count * 100,
        }
    }
}

/// Cost estimate for a retrieval operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct CostEstimate {
    /// Estimated number of LLM calls.
    pub llm_calls: usize,
    /// Estimated number of tokens.
    pub tokens: usize,
}

/// Context passed to strategies during retrieval.
#[derive(Debug, Clone)]
pub struct RetrievalContext {
    /// The original query.
    pub query: String,
    /// Normalized/lowercase query for matching.
    pub query_normalized: String,
    /// Query tokens for keyword matching.
    pub query_tokens: Vec<String>,
    /// Current depth in the tree.
    pub current_depth: usize,
    /// Number of results collected so far.
    pub results_count: usize,
    /// Total tokens collected so far.
    pub tokens_collected: usize,
    /// Maximum tokens allowed.
    pub max_tokens: usize,
    /// Whether sufficiency check is enabled.
    pub sufficiency_enabled: bool,
}

impl RetrievalContext {
    /// Create a new retrieval context from a query.
    pub fn new(query: &str, max_tokens: usize, sufficiency_enabled: bool) -> Self {
        let query_normalized = query.to_lowercase();
        let query_tokens: Vec<String> = query_normalized
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        Self {
            query: query.to_string(),
            query_normalized,
            query_tokens,
            current_depth: 0,
            results_count: 0,
            tokens_collected: 0,
            max_tokens,
            sufficiency_enabled,
        }
    }

    /// Check if we've reached the token limit.
    pub fn is_token_limit_reached(&self) -> bool {
        self.tokens_collected >= self.max_tokens
    }

    /// Calculate token utilization percentage.
    pub fn token_utilization(&self) -> f32 {
        if self.max_tokens == 0 {
            0.0
        } else {
            (self.tokens_collected as f32 / self.max_tokens as f32).min(1.0)
        }
    }
}
