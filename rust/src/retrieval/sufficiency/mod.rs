// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Sufficiency checking for incremental retrieval.
//!
//! Determines when enough information has been collected to answer the query.

mod llm_judge;
mod threshold;

pub use super::types::SufficiencyLevel;

/// Trait for sufficiency checking strategies.
pub trait SufficiencyChecker: Send + Sync {
    /// Check if the collected content is sufficient to answer the query.
    ///
    /// # Arguments
    ///
    /// * `query` - The original query
    /// * `content` - The collected content so far
    /// * `token_count` - Approximate token count of content
    ///
    /// # Returns
    ///
    /// A `SufficiencyLevel` indicating whether to continue retrieving.
    fn check(&self, query: &str, content: &str, token_count: usize) -> SufficiencyLevel;

    /// Get the name of this checker.
    fn name(&self) -> &str;
}
