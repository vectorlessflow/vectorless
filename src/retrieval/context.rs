// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Context building for retrieval results.
//!
//! This module provides utilities for building context strings
//! from retrieval results for LLM consumption.
//!
//! # Features
//!
//! - Multiple pruning strategies (token-based, relevance-based, diversity)
//! - Configurable token estimation (fast or accurate)
//! - Async support for large documents
//!
//! # Example
//!
//! ```rust,ignore
//! // Synchronous
//! let context = ContextBuilder::new()
//!     .with_max_tokens(4000)
//!     .with_pruning_strategy(PruningStrategy::Hybrid { min_relevance: 0.5 })
//!     .build(&results);
//!
//! // Asynchronous (for large documents)
//! let context = ContextBuilder::new()
//!     .with_max_tokens(4000)
//!     .build_async(&results).await?;
//! ```

use super::types::RetrievalResult;
use crate::domain::{DocumentTree, NodeId, estimate_tokens};
use std::collections::HashSet;

/// Pruning strategy for context building.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PruningStrategy {
    /// Stop when token limit is reached (default).
    TokenLimit,
    /// Keep only results above relevance threshold.
    RelevanceThreshold(f32),
    /// Diversity-based: avoid redundant content.
    Diversity {
        /// Maximum keyword overlap ratio (0.0-1.0).
        max_overlap: f32,
    },
    /// Combined: token limit with relevance filtering.
    Hybrid {
        /// Minimum relevance score to include.
        min_relevance: f32,
    },
}

impl Default for PruningStrategy {
    fn default() -> Self {
        Self::TokenLimit
    }
}

/// Token estimation mode.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TokenEstimation {
    /// Fast estimation: ~4 chars per token.
    #[default]
    Fast,
    /// Accurate estimation using tiktoken.
    Accurate,
}

/// Context builder for assembling retrieval results.
#[derive(Debug)]
pub struct ContextBuilder {
    /// Maximum tokens for the context.
    max_tokens: usize,

    /// Whether to include titles.
    include_titles: bool,

    /// Whether to include summaries.
    include_summaries: bool,

    /// Whether to include content.
    include_content: bool,

    /// Separator between sections.
    separator: String,

    /// Pruning strategy.
    pruning_strategy: PruningStrategy,

    /// Token estimation mode.
    token_estimation: TokenEstimation,

    /// Chunk size for async processing.
    async_chunk_size: usize,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextBuilder {
    /// Create a new context builder.
    pub fn new() -> Self {
        Self {
            max_tokens: 4000,
            include_titles: true,
            include_summaries: true,
            include_content: true,
            separator: "\n\n---\n\n".to_string(),
            pruning_strategy: PruningStrategy::TokenLimit,
            token_estimation: TokenEstimation::Fast,
            async_chunk_size: 100,
        }
    }

    /// Set the maximum tokens.
    pub fn with_max_tokens(mut self, tokens: usize) -> Self {
        self.max_tokens = tokens;
        self
    }

    /// Set whether to include titles.
    pub fn with_titles(mut self, include: bool) -> Self {
        self.include_titles = include;
        self
    }

    /// Set whether to include summaries.
    pub fn with_summaries(mut self, include: bool) -> Self {
        self.include_summaries = include;
        self
    }

    /// Set whether to include content.
    pub fn with_content(mut self, include: bool) -> Self {
        self.include_content = include;
        self
    }

    /// Set the separator.
    pub fn with_separator(mut self, separator: impl Into<String>) -> Self {
        self.separator = separator.into();
        self
    }

    /// Set the pruning strategy.
    pub fn with_pruning_strategy(mut self, strategy: PruningStrategy) -> Self {
        self.pruning_strategy = strategy;
        self
    }

    /// Set token estimation mode.
    pub fn with_token_estimation(mut self, mode: TokenEstimation) -> Self {
        self.token_estimation = mode;
        self
    }

    /// Set chunk size for async processing.
    pub fn with_async_chunk_size(mut self, size: usize) -> Self {
        self.async_chunk_size = size;
        self
    }

    /// Estimate tokens for a string.
    fn estimate_tokens(&self, text: &str) -> usize {
        match self.token_estimation {
            TokenEstimation::Fast => text.len() / 4,
            TokenEstimation::Accurate => estimate_tokens(text),
        }
    }

    /// Build context from retrieval results (synchronous).
    pub fn build(&self, results: &[RetrievalResult]) -> String {
        match self.pruning_strategy {
            PruningStrategy::TokenLimit => self.build_token_limit(results),
            PruningStrategy::RelevanceThreshold(min) => self.build_relevance(results, min),
            PruningStrategy::Diversity { max_overlap } => {
                self.build_diversity(results, max_overlap)
            }
            PruningStrategy::Hybrid { min_relevance } => self.build_hybrid(results, min_relevance),
        }
    }

    /// Build context asynchronously for large documents.
    ///
    /// Processes results in chunks to avoid blocking.
    pub async fn build_async(&self, results: &[RetrievalResult]) -> String {
        // For small result sets, just use sync
        if results.len() < self.async_chunk_size {
            return self.build(results);
        }

        // Process in chunks with yield points
        let mut sections = Vec::new();
        let mut estimated_tokens = 0;
        let separator_tokens = self.estimate_tokens(&self.separator);
        let mut included_keywords: HashSet<String> = HashSet::new();

        for (i, chunk) in results.chunks(self.async_chunk_size).enumerate() {
            // Yield to the runtime every chunk
            if i > 0 {
                tokio::task::yield_now().await;
            }

            for result in chunk {
                // Apply pruning strategy
                match self.pruning_strategy {
                    PruningStrategy::RelevanceThreshold(min) => {
                        if result.score < min {
                            continue;
                        }
                    }
                    PruningStrategy::Diversity { max_overlap } => {
                        let keywords = self.extract_keywords(result);
                        if self.calculate_overlap(&keywords, &included_keywords) > max_overlap {
                            continue;
                        }
                        included_keywords.extend(keywords);
                    }
                    PruningStrategy::Hybrid { min_relevance } => {
                        if result.score < min_relevance {
                            continue;
                        }
                    }
                    PruningStrategy::TokenLimit => {}
                }

                let section = self.format_section(result);
                let section_tokens = self.estimate_tokens(&section);

                if estimated_tokens + section_tokens + separator_tokens > self.max_tokens {
                    break;
                }

                estimated_tokens += section_tokens + separator_tokens;
                sections.push(section);
            }

            // Early exit if we've hit the token limit
            if estimated_tokens >= self.max_tokens {
                break;
            }
        }

        sections.join(&self.separator)
    }

    /// Build with simple token limit.
    fn build_token_limit(&self, results: &[RetrievalResult]) -> String {
        let mut sections = Vec::new();
        let mut estimated_tokens = 0;
        let separator_tokens = self.estimate_tokens(&self.separator);

        for result in results {
            let section = self.format_section(result);
            let section_tokens = self.estimate_tokens(&section);

            if estimated_tokens + section_tokens + separator_tokens > self.max_tokens {
                break;
            }

            estimated_tokens += section_tokens + separator_tokens;
            sections.push(section);
        }

        sections.join(&self.separator)
    }

    /// Build with relevance threshold.
    fn build_relevance(&self, results: &[RetrievalResult], min_score: f32) -> String {
        let mut sections = Vec::new();
        let mut estimated_tokens = 0;
        let separator_tokens = self.estimate_tokens(&self.separator);

        for result in results {
            if result.score < min_score {
                continue;
            }

            let section = self.format_section(result);
            let section_tokens = self.estimate_tokens(&section);

            if estimated_tokens + section_tokens + separator_tokens > self.max_tokens {
                break;
            }

            estimated_tokens += section_tokens + separator_tokens;
            sections.push(section);
        }

        sections.join(&self.separator)
    }

    /// Build with diversity-based pruning.
    fn build_diversity(&self, results: &[RetrievalResult], max_overlap: f32) -> String {
        let mut sections = Vec::new();
        let mut estimated_tokens = 0;
        let separator_tokens = self.estimate_tokens(&self.separator);
        let mut included_keywords: HashSet<String> = HashSet::new();

        for result in results {
            let keywords = self.extract_keywords(result);

            if self.calculate_overlap(&keywords, &included_keywords) > max_overlap {
                continue;
            }

            let section = self.format_section(result);
            let section_tokens = self.estimate_tokens(&section);

            if estimated_tokens + section_tokens + separator_tokens > self.max_tokens {
                break;
            }

            estimated_tokens += section_tokens + separator_tokens;
            included_keywords.extend(keywords);
            sections.push(section);
        }

        sections.join(&self.separator)
    }

    /// Build with hybrid strategy (relevance + token limit).
    fn build_hybrid(&self, results: &[RetrievalResult], min_relevance: f32) -> String {
        let mut sections = Vec::new();
        let mut estimated_tokens = 0;
        let separator_tokens = self.estimate_tokens(&self.separator);

        for result in results {
            if result.score < min_relevance {
                continue;
            }

            let section = self.format_section(result);
            let section_tokens = self.estimate_tokens(&section);

            if estimated_tokens + section_tokens + separator_tokens > self.max_tokens {
                break;
            }

            estimated_tokens += section_tokens + separator_tokens;
            sections.push(section);
        }

        sections.join(&self.separator)
    }

    /// Extract keywords from a result for diversity checking.
    fn extract_keywords(&self, result: &RetrievalResult) -> Vec<String> {
        let mut words = Vec::new();

        // Collect from title
        words.extend(
            result
                .title
                .to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .map(|w| w.to_string()),
        );

        // Collect from summary
        if let Some(summary) = &result.summary {
            words.extend(
                summary
                    .to_lowercase()
                    .split_whitespace()
                    .filter(|w| w.len() > 3)
                    .map(|w| w.to_string()),
            );
        }

        // Limit keywords
        words.truncate(20);
        words
    }

    /// Calculate overlap between keyword sets.
    fn calculate_overlap(&self, new_keywords: &[String], existing: &HashSet<String>) -> f32 {
        if new_keywords.is_empty() || existing.is_empty() {
            return 0.0;
        }

        let matches = new_keywords
            .iter()
            .filter(|k| existing.contains(*k))
            .count();

        matches as f32 / new_keywords.len() as f32
    }

    /// Build context from a document tree starting at a node (synchronous).
    pub fn build_from_tree(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        max_depth: usize,
    ) -> String {
        let mut sections = Vec::new();
        self.collect_sections(tree, node_id, 0, max_depth, &mut sections);
        sections.join(&self.separator)
    }

    /// Build context from a document tree asynchronously.
    pub async fn build_from_tree_async(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        max_depth: usize,
    ) -> String {
        let mut sections = Vec::new();
        self.collect_sections_async(tree, node_id, 0, max_depth, &mut sections)
            .await;
        sections.join(&self.separator)
    }

    fn collect_sections(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        current_depth: usize,
        max_depth: usize,
        sections: &mut Vec<String>,
    ) {
        if current_depth > max_depth {
            return;
        }

        if let Some(node) = tree.get(node_id) {
            let section = self.format_node_section(node, current_depth);
            if !section.is_empty() {
                sections.push(section);
            }

            for child_id in tree.children_iter(node_id) {
                self.collect_sections(tree, child_id, current_depth + 1, max_depth, sections);
            }
        }
    }

    async fn collect_sections_async(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        current_depth: usize,
        max_depth: usize,
        sections: &mut Vec<String>,
    ) {
        if current_depth > max_depth {
            return;
        }

        // Yield every few levels to avoid blocking
        if current_depth > 0 && current_depth.is_multiple_of(3) {
            tokio::task::yield_now().await;
        }

        if let Some(node) = tree.get(node_id) {
            let section = self.format_node_section(node, current_depth);
            if !section.is_empty() {
                sections.push(section);
            }

            for child_id in tree.children_iter(node_id) {
                Box::pin(self.collect_sections_async(
                    tree,
                    child_id,
                    current_depth + 1,
                    max_depth,
                    sections,
                ))
                .await;
            }
        }
    }

    fn format_node_section(&self, node: &crate::domain::TreeNode, depth: usize) -> String {
        let mut section = String::new();

        if self.include_titles {
            let indent = "  ".repeat(depth);
            section.push_str(&format!("{}# {}\n", indent, node.title));
        }

        if self.include_summaries && !node.summary.is_empty() {
            section.push_str(&format!("Summary: {}\n", node.summary));
        }

        if self.include_content && !node.content.is_empty() {
            section.push_str(&format!("\n{}\n", node.content));
        }

        section
    }

    fn format_section(&self, result: &RetrievalResult) -> String {
        let mut section = String::new();

        if self.include_titles {
            section.push_str(&format!("## {}\n", result.title));
        }

        if self.include_summaries {
            if let Some(summary) = &result.summary {
                section.push_str(&format!("Summary: {}\n", summary));
            }
        }

        if self.include_content {
            if let Some(content) = &result.content {
                section.push_str(&format!("\n{}\n", content));
            }
        }

        section
    }
}

/// Format retrieval results for LLM consumption.
pub fn format_for_llm(results: &[RetrievalResult], max_tokens: usize) -> String {
    ContextBuilder::new()
        .with_max_tokens(max_tokens)
        .build(results)
}

/// Format retrieval results asynchronously.
pub async fn format_for_llm_async(results: &[RetrievalResult], max_tokens: usize) -> String {
    ContextBuilder::new()
        .with_max_tokens(max_tokens)
        .build_async(results)
        .await
}

/// Format a document tree for LLM consumption.
pub fn format_tree_for_llm(tree: &DocumentTree, max_depth: usize, max_tokens: usize) -> String {
    ContextBuilder::new()
        .with_max_tokens(max_tokens)
        .build_from_tree(tree, tree.root(), max_depth)
}

/// Format a document tree asynchronously.
pub async fn format_tree_for_llm_async(
    tree: &DocumentTree,
    max_depth: usize,
    max_tokens: usize,
) -> String {
    ContextBuilder::new()
        .with_max_tokens(max_tokens)
        .build_from_tree_async(tree, tree.root(), max_depth)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_builder() {
        let results = vec![
            RetrievalResult::new("Section 1").with_content("Content 1"),
            RetrievalResult::new("Section 2").with_content("Content 2"),
        ];

        let context = ContextBuilder::new().with_max_tokens(1000).build(&results);

        assert!(context.contains("Section 1"));
        assert!(context.contains("Content 1"));
    }

    #[test]
    fn test_pruning_strategy_relevance() {
        let results = vec![
            RetrievalResult::new("High relevance").with_score(0.9),
            RetrievalResult::new("Low relevance").with_score(0.1),
        ];

        let context = ContextBuilder::new()
            .with_max_tokens(1000)
            .with_pruning_strategy(PruningStrategy::RelevanceThreshold(0.5))
            .build(&results);

        assert!(context.contains("High relevance"));
        assert!(!context.contains("Low relevance"));
    }

    #[test]
    fn test_token_estimation_modes() {
        let fast_builder = ContextBuilder::new().with_token_estimation(TokenEstimation::Fast);
        let accurate_builder =
            ContextBuilder::new().with_token_estimation(TokenEstimation::Accurate);

        let fast_tokens = fast_builder.estimate_tokens("Hello world test");
        let accurate_tokens = accurate_builder.estimate_tokens("Hello world test");

        assert!(fast_tokens > 0);
        assert!(accurate_tokens > 0);
    }

    #[test]
    fn test_diversity_pruning() {
        let results = vec![
            RetrievalResult::new("Unique topic alpha").with_score(0.9),
            RetrievalResult::new("Unique topic alpha beta").with_score(0.8), // Similar
            RetrievalResult::new("Different gamma delta").with_score(0.7),
        ];

        let context = ContextBuilder::new()
            .with_max_tokens(1000)
            .with_pruning_strategy(PruningStrategy::Diversity { max_overlap: 0.3 })
            .build(&results);

        // Should include first and third, skip second (too similar to first)
        assert!(context.contains("alpha"));
        assert!(context.contains("gamma"));
    }

    #[tokio::test]
    async fn test_async_build() {
        let results: Vec<_> = (0..200)
            .map(|i| {
                RetrievalResult::new(&format!("Section {}", i))
                    .with_content(&format!("Content {}", i))
            })
            .collect();

        let context = ContextBuilder::new()
            .with_max_tokens(10000)
            .build_async(&results)
            .await;

        assert!(!context.is_empty());
    }
}
