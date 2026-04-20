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
    /// Per-operation timeout (seconds). `None` means no timeout.
    pub(crate) timeout_secs: Option<u64>,
    /// Force Orchestrator analysis even when documents are specified.
    ///
    /// When `true`, the Orchestrator analyzes DocCards to select relevant
    /// documents instead of dispatching all specified docs directly.
    /// Useful when the user wants the system to decide which documents
    /// (or sections) are most relevant to the query.
    pub(crate) force_analysis: bool,
}

impl QueryContext {
    /// Create a new query context (defaults to workspace scope).
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            scope: QueryScope::Workspace,
            timeout_secs: None,
            force_analysis: false,
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

    /// Set per-operation timeout in seconds.
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Force the Orchestrator to analyze documents before dispatching Workers.
    ///
    /// By default, when documents are specified via `with_doc_ids()`, the
    /// Orchestrator skips its analysis phase and dispatches Workers to all
    /// specified documents directly. Setting this to `true` forces the
    /// Orchestrator to analyze DocCards and decide which documents are
    /// relevant, even when the user specified documents explicitly.
    ///
    /// This is useful when querying across many documents where only a subset
    /// is likely relevant to the specific question.
    pub fn with_force_analysis(mut self, force: bool) -> Self {
        self.force_analysis = force;
        self
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
            .with_timeout_secs(60);

        assert_eq!(ctx.timeout_secs, Some(60));
    }

    #[test]
    fn test_query_context_timeout_default() {
        let ctx = QueryContext::new("test");
        assert_eq!(ctx.timeout_secs, None);
    }
}
