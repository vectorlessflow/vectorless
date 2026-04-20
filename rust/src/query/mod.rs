// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Query understanding and planning.
//!
//! Analyzes a user's raw query and produces a structured [`QueryPlan`]
//! for downstream modules (Orchestrator, Worker).
//!
//! # Pipeline
//!
//! ```text
//! raw query string
//!   → extract keywords            (from scoring/bm25)
//!   → LLM query understanding     (intent, concepts, complexity)
//!   → QueryPlan
//! ```
//!
//! On LLM failure, falls back to keyword-only analysis.

mod text;
mod types;
mod understand;

#[allow(unused_imports)]
pub use types::{Complexity, QueryIntent, QueryPlan, SubQuery};

use crate::llm::LlmClient;
use crate::scoring::bm25::extract_keywords;

/// Query understanding pipeline.
///
/// Produces a [`QueryPlan`] from a raw query string.
/// Uses LLM for deep understanding with graceful fallback.
pub struct QueryPipeline;

impl QueryPipeline {
    /// Analyze a query and produce a structured plan.
    ///
    /// 1. Extract keywords (zero-cost, no LLM)
    /// 2. LLM deep understanding (intent, concepts, complexity)
    /// 3. Graceful fallback to keyword-only plan on LLM failure
    pub async fn understand(
        query: &str,
        llm: &LlmClient,
    ) -> crate::error::Result<QueryPlan> {
        let keywords = extract_keywords(query);
        understand::understand(query, &keywords, llm).await
    }
}
