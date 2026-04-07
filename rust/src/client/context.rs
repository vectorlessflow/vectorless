// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Request context and configuration.
//!
//! This module provides request-scoped configuration and state management
//! for client operations. It allows overriding global configuration on a
//! per-request basis.
//!
//! # Example
//!
//! ```rust,ignore
//! let ctx = ClientContext::new()
//!     .with_top_k(10)
//!     .with_token_budget(8000)
//!     .with_timeout(Duration::from_secs(30));
//!
//! let result = client.query_with_context(&doc_id, "query", &ctx).await?;
//! ```

use std::collections::HashMap;
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::retrieval::content::OutputFormatConfig;

/// Request context for client operations.
///
/// Provides request-scoped configuration overrides and metadata.
#[derive(Debug, Clone)]
pub struct ClientContext {
    /// Unique request ID for tracing.
    pub request_id: Uuid,

    /// Request-specific configuration overrides.
    pub config: RequestContextConfig,

    /// Request metadata (custom key-value pairs).
    pub metadata: HashMap<String, String>,

    /// Request deadline (for timeout).
    pub deadline: Option<Instant>,

    /// Priority (higher = more important).
    pub priority: u8,
}

impl Default for ClientContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientContext {
    /// Create a new context with defaults.
    pub fn new() -> Self {
        Self {
            request_id: Uuid::new_v4(),
            config: RequestContextConfig::default(),
            metadata: HashMap::new(),
            deadline: None,
            priority: 5, // Default priority
        }
    }

    /// Create a context with a specific request ID.
    pub fn with_id(id: Uuid) -> Self {
        Self {
            request_id: id,
            ..Self::new()
        }
    }

    /// Set the top_k override for retrieval.
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.config.top_k = Some(top_k);
        self
    }

    /// Set the token budget override.
    pub fn with_token_budget(mut self, budget: usize) -> Self {
        self.config.token_budget = Some(budget);
        self
    }

    /// Set the content format override.
    pub fn with_content_format(mut self, format: OutputFormatConfig) -> Self {
        self.config.content_format = Some(format);
        self
    }

    /// Set whether to include summaries.
    pub fn with_summaries(mut self, include: bool) -> Self {
        self.config.features.include_summaries = include;
        self
    }

    /// Set whether to include content.
    pub fn with_content(mut self, include: bool) -> Self {
        self.config.features.include_content = include;
        self
    }

    /// Set whether to enable caching.
    pub fn with_cache(mut self, enable: bool) -> Self {
        self.config.features.enable_cache = enable;
        self
    }

    /// Set whether to enable sufficiency checking.
    pub fn with_sufficiency_check(mut self, enable: bool) -> Self {
        self.config.features.enable_sufficiency_check = enable;
        self
    }

    /// Set a timeout duration.
    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.deadline = Some(Instant::now() + duration);
        self
    }

    /// Set a deadline.
    pub fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Set the priority (0-10, higher = more important).
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority.min(10);
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Check if the request has timed out.
    pub fn is_timed_out(&self) -> bool {
        self.deadline.map(|d| Instant::now() > d).unwrap_or(false)
    }

    /// Get remaining time until deadline.
    pub fn remaining_time(&self) -> Option<Duration> {
        self.deadline
            .map(|d| d.saturating_duration_since(Instant::now()))
    }

    /// Merge with another context (other takes precedence).
    pub fn merge(&self, other: &ClientContext) -> ClientContext {
        let mut merged = self.clone();
        merged.request_id = other.request_id;

        if other.config.top_k.is_some() {
            merged.config.top_k = other.config.top_k;
        }
        if other.config.token_budget.is_some() {
            merged.config.token_budget = other.config.token_budget;
        }
        if other.config.content_format.is_some() {
            merged.config.content_format = other.config.content_format.clone();
        }
        if other.deadline.is_some() {
            merged.deadline = other.deadline;
        }
        if other.priority != 5 {
            merged.priority = other.priority;
        }

        // Merge metadata
        for (k, v) in &other.metadata {
            merged.metadata.insert(k.clone(), v.clone());
        }

        // Merge feature flags
        merged.config.features = FeatureFlags {
            include_summaries: other.config.features.include_summaries,
            include_content: other.config.features.include_content,
            enable_cache: other.config.features.enable_cache,
            enable_sufficiency_check: other.config.features.enable_sufficiency_check,
        };

        merged
    }
}

/// Request-specific configuration overrides.
#[derive(Debug, Clone, Default)]
pub struct RequestContextConfig {
    /// Override top_k for retrieval.
    pub top_k: Option<usize>,

    /// Override token budget.
    pub token_budget: Option<usize>,

    /// Override content format.
    pub content_format: Option<OutputFormatConfig>,

    /// Feature flags.
    pub features: FeatureFlags,
}

/// Feature flags for request.
#[derive(Debug, Clone, Copy)]
pub struct FeatureFlags {
    /// Include summaries in results.
    pub include_summaries: bool,

    /// Include content in results.
    pub include_content: bool,

    /// Enable result caching.
    pub enable_cache: bool,

    /// Enable sufficiency checking.
    pub enable_sufficiency_check: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            include_summaries: true,
            include_content: true,
            enable_cache: true,
            enable_sufficiency_check: true,
        }
    }
}

impl FeatureFlags {
    /// Create with all features enabled.
    pub fn all() -> Self {
        Self {
            include_summaries: true,
            include_content: true,
            enable_cache: true,
            enable_sufficiency_check: true,
        }
    }

    /// Create with minimal features (fastest).
    pub fn minimal() -> Self {
        Self {
            include_summaries: false,
            include_content: true,
            enable_cache: false,
            enable_sufficiency_check: false,
        }
    }

    /// Create for deep analysis.
    pub fn deep() -> Self {
        Self {
            include_summaries: true,
            include_content: true,
            enable_cache: true,
            enable_sufficiency_check: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = ClientContext::new();
        assert!(!ctx.request_id.is_nil());
        assert!(ctx.config.top_k.is_none());
        assert!(ctx.deadline.is_none());
    }

    #[test]
    fn test_context_with_overrides() {
        let ctx = ClientContext::new()
            .with_top_k(10)
            .with_token_budget(8000)
            .with_cache(false);

        assert_eq!(ctx.config.top_k, Some(10));
        assert_eq!(ctx.config.token_budget, Some(8000));
        assert!(!ctx.config.features.enable_cache);
    }

    #[test]
    fn test_context_timeout() {
        let ctx = ClientContext::new().with_timeout(Duration::from_millis(100));

        assert!(!ctx.is_timed_out());
        assert!(ctx.remaining_time().is_some());
    }

    #[test]
    fn test_context_metadata() {
        let ctx = ClientContext::new()
            .with_metadata("user", "test")
            .with_metadata("version", "1.0");

        assert_eq!(ctx.metadata.get("user"), Some(&"test".to_string()));
        assert_eq!(ctx.metadata.get("version"), Some(&"1.0".to_string()));
    }

    #[test]
    fn test_context_merge() {
        let ctx1 = ClientContext::new()
            .with_top_k(5)
            .with_metadata("key1", "value1");

        let ctx2 = ClientContext::new()
            .with_top_k(10)
            .with_metadata("key2", "value2");

        let merged = ctx1.merge(&ctx2);

        assert_eq!(merged.config.top_k, Some(10));
        assert_eq!(merged.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(merged.metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_feature_flags() {
        let all = FeatureFlags::all();
        assert!(all.include_summaries);
        assert!(all.include_content);

        let minimal = FeatureFlags::minimal();
        assert!(!minimal.include_summaries);
        assert!(!minimal.enable_cache);
    }
}
