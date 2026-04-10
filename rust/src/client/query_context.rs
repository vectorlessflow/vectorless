// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Query context for the Engine API.
//!
//! [`QueryContext`] encapsulates all parameters for a query operation,
//! providing a builder pattern for configuration.
//!
//! # Example
//!
//! ```rust
//! use vectorless::client::QueryContext;
//!
//! // Simple query
//! let ctx = QueryContext::new("What is the total revenue?");
//!
//! // With document scope
//! let ctx = QueryContext::new("What is the architecture?")
//!     .with_doc_id("doc-abc123");
//!
//! // With options
//! let ctx = QueryContext::new("Explain the algorithm")
//!     .with_doc_id("doc-abc123")
//!     .with_max_tokens(4000);
//! ```

use crate::config::Config;
use crate::retrieval::{RetrieveOptions, StrategyPreference};

/// Context for a query operation.
///
/// Encapsulates the query text, target document, and retrieval options.
/// Use builder methods to configure.
///
/// # Convenience
///
/// Implements `From<String>` and `From<&str>` for quick construction:
///
/// ```rust
/// use vectorless::client::QueryContext;
///
/// let ctx: QueryContext = "What is this?".into();
/// ```
#[derive(Debug, Clone)]
pub struct QueryContext {
    /// The query text.
    pub(crate) query: String,
    /// Target document ID. None means query all (not yet supported).
    pub(crate) doc_id: Option<String>,
    /// Maximum tokens for the result content.
    pub(crate) max_tokens: Option<usize>,
    /// Retrieval strategy override.
    pub(crate) strategy: Option<StrategyPreference>,
    /// Whether to include the reasoning chain in the result.
    pub(crate) include_reasoning: bool,
    /// Maximum tree traversal depth.
    pub(crate) depth_limit: Option<usize>,
}

impl QueryContext {
    /// Create a new query context with the given query text.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            doc_id: None,
            max_tokens: None,
            strategy: None,
            include_reasoning: true,
            depth_limit: None,
        }
    }

    /// Set the target document ID.
    pub fn with_doc_id(mut self, doc_id: impl Into<String>) -> Self {
        self.doc_id = Some(doc_id.into());
        self
    }

    /// Set the maximum tokens for the result content.
    pub fn with_max_tokens(mut self, tokens: usize) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// Set the retrieval strategy.
    pub fn with_strategy(mut self, strategy: StrategyPreference) -> Self {
        self.strategy = Some(strategy);
        self
    }

    /// Set whether to include the reasoning chain.
    pub fn with_include_reasoning(mut self, include: bool) -> Self {
        self.include_reasoning = include;
        self
    }

    /// Set the maximum tree traversal depth.
    pub fn with_depth_limit(mut self, depth: usize) -> Self {
        self.depth_limit = Some(depth);
        self
    }

    /// Convert to internal `RetrieveOptions`, merging with engine config.
    pub(crate) fn to_retrieve_options(&self, config: &Config) -> RetrieveOptions {
        let mut opts = RetrieveOptions::new()
            .with_top_k(config.retrieval.top_k)
            .with_include_content(true)
            .with_include_summaries(true);

        if let Some(max_tokens) = self.max_tokens {
            opts = opts.with_max_tokens(max_tokens);
        }

        if let Some(strategy) = &self.strategy {
            opts = opts.with_strategy(strategy.clone());
        }

        opts
    }
}

impl From<String> for QueryContext {
    fn from(query: String) -> Self {
        Self::new(query)
    }
}

impl From<&str> for QueryContext {
    fn from(query: &str) -> Self {
        Self::new(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_context_new() {
        let ctx = QueryContext::new("What is this?");
        assert_eq!(ctx.query, "What is this?");
        assert!(ctx.doc_id.is_none());
        assert!(ctx.include_reasoning);
    }

    #[test]
    fn test_query_context_from_string() {
        let ctx: QueryContext = "Hello".to_string().into();
        assert_eq!(ctx.query, "Hello");
    }

    #[test]
    fn test_query_context_from_str() {
        let ctx: QueryContext = "Hello".into();
        assert_eq!(ctx.query, "Hello");
    }

    #[test]
    fn test_query_context_builder() {
        let ctx = QueryContext::new("test")
            .with_doc_id("doc-1")
            .with_max_tokens(4000)
            .with_include_reasoning(false)
            .with_depth_limit(5);

        assert_eq!(ctx.doc_id, Some("doc-1".to_string()));
        assert_eq!(ctx.max_tokens, Some(4000));
        assert!(!ctx.include_reasoning);
        assert_eq!(ctx.depth_limit, Some(5));
    }
}
