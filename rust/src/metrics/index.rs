// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Indexing pipeline metrics.

use serde::{Deserialize, Serialize};

/// Performance metrics for the indexing pipeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexMetrics {
    /// Parse stage duration (ms).
    #[serde(default)]
    pub parse_time_ms: u64,

    /// Build stage duration (ms).
    #[serde(default)]
    pub build_time_ms: u64,

    /// Enhance stage duration (ms).
    #[serde(default)]
    pub enhance_time_ms: u64,

    /// Enrich stage duration (ms).
    #[serde(default)]
    pub enrich_time_ms: u64,

    /// Optimize stage duration (ms).
    #[serde(default)]
    pub optimize_time_ms: u64,

    /// Validate stage duration (ms).
    #[serde(default)]
    pub validate_time_ms: u64,

    /// Split stage duration (ms).
    #[serde(default)]
    pub split_time_ms: u64,

    /// Reasoning index build duration (ms).
    #[serde(default)]
    pub reasoning_index_time_ms: u64,

    /// Navigation index build duration (ms).
    #[serde(default)]
    pub navigation_index_time_ms: u64,

    /// Number of nav entries in navigation index.
    #[serde(default)]
    pub nav_entries_indexed: usize,

    /// Number of child routes in navigation index.
    #[serde(default)]
    pub child_routes_indexed: usize,

    /// Number of topics indexed in reasoning index.
    #[serde(default)]
    pub topics_indexed: usize,

    /// Number of keywords indexed in reasoning index.
    #[serde(default)]
    pub keywords_indexed: usize,

    /// Total tokens generated (summaries).
    #[serde(default)]
    pub total_tokens_generated: usize,

    /// Number of LLM calls.
    #[serde(default)]
    pub llm_calls: usize,

    /// Number of nodes processed.
    #[serde(default)]
    pub nodes_processed: usize,

    /// Number of summaries generated.
    #[serde(default)]
    pub summaries_generated: usize,

    /// Number of summaries that failed to generate (LLM error, rate limit, etc.).
    #[serde(default)]
    pub summaries_failed: usize,

    /// Number of nodes skipped (thinning).
    #[serde(default)]
    pub nodes_skipped: usize,

    /// Number of nodes merged.
    #[serde(default)]
    pub nodes_merged: usize,
}

impl IndexMetrics {
    /// Create new metrics with start time.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record parse stage time.
    pub fn record_parse(&mut self, duration_ms: u64) {
        self.parse_time_ms = duration_ms;
    }

    /// Record build stage time.
    pub fn record_build(&mut self, duration_ms: u64) {
        self.build_time_ms = duration_ms;
    }

    /// Record enhance stage time.
    pub fn record_enhance(&mut self, duration_ms: u64) {
        self.enhance_time_ms = duration_ms;
    }

    /// Record enrich stage time.
    pub fn record_enrich(&mut self, duration_ms: u64) {
        self.enrich_time_ms = duration_ms;
    }

    /// Record optimize stage time.
    pub fn record_optimize(&mut self, duration_ms: u64) {
        self.optimize_time_ms = duration_ms;
    }

    /// Record validate stage time.
    pub fn record_validate(&mut self, duration_ms: u64) {
        self.validate_time_ms = duration_ms;
    }

    /// Record split stage time.
    pub fn record_split(&mut self, duration_ms: u64) {
        self.split_time_ms = duration_ms;
    }

    /// Record reasoning index build time.
    pub fn record_reasoning_index(&mut self, duration_ms: u64, topics: usize, keywords: usize) {
        self.reasoning_index_time_ms = duration_ms;
        self.topics_indexed = topics;
        self.keywords_indexed = keywords;
    }

    /// Record navigation index build time.
    pub fn record_navigation_index(
        &mut self,
        duration_ms: u64,
        nav_entries: usize,
        child_routes: usize,
    ) {
        self.navigation_index_time_ms = duration_ms;
        self.nav_entries_indexed = nav_entries;
        self.child_routes_indexed = child_routes;
    }

    /// Increment LLM calls.
    pub fn increment_llm_calls(&mut self) {
        self.llm_calls += 1;
    }

    /// Add to tokens generated.
    pub fn add_tokens_generated(&mut self, tokens: usize) {
        self.total_tokens_generated += tokens;
    }

    /// Set nodes processed.
    pub fn set_nodes_processed(&mut self, count: usize) {
        self.nodes_processed = count;
    }

    /// Increment summaries generated.
    pub fn increment_summaries(&mut self) {
        self.summaries_generated += 1;
    }

    /// Add to summaries failed count.
    pub fn add_summaries_failed(&mut self, count: usize) {
        self.summaries_failed += count;
    }

    /// Increment nodes skipped.
    pub fn increment_nodes_skipped(&mut self) {
        self.nodes_skipped += 1;
    }

    /// Increment nodes merged.
    pub fn increment_nodes_merged(&mut self) {
        self.nodes_merged += 1;
    }

    /// Get total time.
    pub fn total_time_ms(&self) -> u64 {
        self.parse_time_ms
            + self.build_time_ms
            + self.validate_time_ms
            + self.split_time_ms
            + self.enhance_time_ms
            + self.enrich_time_ms
            + self.reasoning_index_time_ms
            + self.navigation_index_time_ms
            + self.optimize_time_ms
    }
}
