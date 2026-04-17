// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Query context for the Engine API.
//!
//! [`QueryContext`] encapsulates all parameters for a query operation,
//! supporting specific documents or entire workspace queries.
//!
//! # Example
//!
//! ```rust
//! use vectorless::client::QueryContext;
//!
//! // Query specific documents
//! let ctx = QueryContext::new("What is the total revenue?")
//!     .with_doc_ids(vec!["doc-1".to_string()]);
//!
//! // Query entire workspace
//! let ctx = QueryContext::new("Explain the algorithm");
//! ```

use crate::config::Config;
use crate::retrieval::{RetrieveOptions, StrategyPreference};

/// Query scope — determines which documents to search.
#[derive(Debug, Clone)]
pub(crate) enum QueryScope {
    /// Query specific documents.
    Documents(Vec<String>),
    /// Query all documents in the workspace.
    Workspace,
}

/// Context for a query operation.
///
/// Supports two scopes:
/// - **Specific documents** — via `with_doc_ids()`
/// - **Entire workspace** — default when no scope is set
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
    /// Target scope.
    pub(crate) scope: QueryScope,
    /// Maximum tokens for the result content.
    pub(crate) max_tokens: Option<usize>,
    /// Retrieval strategy override.
    pub(crate) strategy: Option<StrategyPreference>,
    /// Whether to include the pilot reasoning chain in the result.
    pub(crate) include_reasoning: bool,
    /// Maximum tree traversal depth for the pilot.
    pub(crate) depth_limit: Option<usize>,
    /// Per-operation timeout (seconds). `None` means no timeout.
    pub(crate) timeout_secs: Option<u64>,
}

impl QueryContext {
    /// Create a new query context (defaults to workspace scope).
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            scope: QueryScope::Workspace,
            max_tokens: None,
            strategy: None,
            include_reasoning: true,
            depth_limit: None,
            timeout_secs: None,
        }
    }

    /// Set scope to specific documents.
    ///
    /// Pass a single ID or multiple IDs to restrict the query
    /// to those documents only.
    pub fn with_doc_ids(mut self, doc_ids: Vec<String>) -> Self {
        self.scope = QueryScope::Documents(doc_ids);
        self
    }

    /// Set scope to entire workspace.
    pub fn with_workspace(mut self) -> Self {
        self.scope = QueryScope::Workspace;
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

    /// Set whether to include the pilot reasoning chain.
    pub fn with_include_reasoning(mut self, include: bool) -> Self {
        self.include_reasoning = include;
        self
    }

    /// Set the maximum tree traversal depth for the pilot.
    pub fn with_depth_limit(mut self, depth: usize) -> Self {
        self.depth_limit = Some(depth);
        self
    }

    /// Set per-operation timeout in seconds.
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
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
    fn test_single_doc_scope() {
        let ctx = QueryContext::new("test").with_doc_ids(vec!["doc-1".to_string()]);
        assert!(
            matches!(ctx.scope, QueryScope::Documents(ref ids) if ids == &["doc-1".to_string()])
        );
    }

    #[test]
    fn test_multi_doc_scope() {
        let ctx = QueryContext::new("test").with_doc_ids(vec!["a".into(), "b".into()]);
        assert!(matches!(ctx.scope, QueryScope::Documents(ref ids) if ids.len() == 2));
    }

    #[test]
    fn test_workspace_scope() {
        let ctx = QueryContext::new("test");
        assert!(matches!(ctx.scope, QueryScope::Workspace));
    }

    #[test]
    fn test_builder_options() {
        let ctx = QueryContext::new("test")
            .with_doc_ids(vec!["doc-1".to_string()])
            .with_max_tokens(4000)
            .with_include_reasoning(false)
            .with_depth_limit(5)
            .with_timeout_secs(60);

        assert_eq!(ctx.max_tokens, Some(4000));
        assert!(!ctx.include_reasoning);
        assert_eq!(ctx.depth_limit, Some(5));
        assert_eq!(ctx.timeout_secs, Some(60));
    }

    #[test]
    fn test_query_context_timeout_default() {
        let ctx = QueryContext::new("test");
        assert_eq!(ctx.timeout_secs, None);
    }
}
