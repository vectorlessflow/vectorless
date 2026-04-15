// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Relevance scoring for content chunks.
//!
//! This module provides fine-grained relevance scoring for content,
//! combining keyword matching, BM25, and optional LLM reranking.

use std::collections::HashMap;

use crate::document::NodeId;
use crate::retrieval::scoring::{Bm25Params, STOPWORDS, extract_keywords};
use crate::utils::estimate_tokens;

use super::config::ScoringStrategyConfig;

/// Content chunk for scoring.
#[derive(Debug, Clone)]
pub struct ContentChunk {
    /// Node ID this chunk belongs to.
    pub node_id: NodeId,
    /// Title of the node.
    pub title: String,
    /// Content text.
    pub content: String,
    /// Depth in tree (0 = root level).
    pub depth: usize,
}

impl ContentChunk {
    /// Create a new content chunk.
    #[must_use]
    pub fn new(node_id: NodeId, title: String, content: String, depth: usize) -> Self {
        Self {
            node_id,
            title,
            content,
            depth,
        }
    }

    /// Estimate token count for this chunk.
    #[must_use]
    pub fn token_count(&self) -> usize {
        estimate_tokens(&self.content)
    }
}

/// Relevance score components.
#[derive(Debug, Clone, Default)]
pub struct ScoreComponents {
    /// Keyword match score (0.0 - 1.0).
    pub keyword_score: f32,
    /// BM25 score (normalized).
    pub bm25_score: f32,
    /// Depth penalty (deeper = lower score).
    pub depth_penalty: f32,
    /// Path bonus from parent relevance.
    pub path_bonus: f32,
    /// Information density score.
    pub density_score: f32,
}

impl ScoreComponents {
    /// Compute final weighted score.
    #[must_use]
    pub fn final_score(&self) -> f32 {
        // Weight formula from design doc
        let score = self.keyword_score * 0.35
            + self.bm25_score * 0.25
            + self.depth_penalty * 0.15
            + self.path_bonus * 0.10
            + self.density_score * 0.15;

        score.clamp(0.0, 1.0)
    }
}

/// Relevance score result for a content chunk.
#[derive(Debug, Clone)]
pub struct ContentRelevance {
    /// The content chunk that was scored.
    pub chunk: ContentChunk,
    /// Final relevance score (0.0 - 1.0).
    pub score: f32,
    /// Score breakdown by component.
    pub components: ScoreComponents,
}

impl ContentRelevance {
    /// Create a new relevance result.
    #[must_use]
    pub fn new(chunk: ContentChunk, score: f32, components: ScoreComponents) -> Self {
        Self {
            chunk,
            score,
            components,
        }
    }
}

/// Context for scoring operations.
#[derive(Debug, Clone)]
pub struct ScoringContext {
    /// Average document length for BM25.
    pub avg_doc_len: f32,
    /// Total document count for IDF.
    pub doc_count: usize,
    /// Document frequency for terms.
    pub doc_freq: HashMap<String, usize>,
    /// Parent node score (for path bonus).
    pub parent_score: Option<f32>,
}

impl Default for ScoringContext {
    fn default() -> Self {
        Self {
            avg_doc_len: 100.0,
            doc_count: 1,
            doc_freq: HashMap::new(),
            parent_score: None,
        }
    }
}

/// Relevance scorer for content chunks.
#[derive(Debug)]
pub struct RelevanceScorer {
    /// Query keywords extracted from the query.
    query_keywords: Vec<String>,
    /// Scoring strategy to use.
    strategy: ScoringStrategyConfig,
    /// BM25 parameters.
    params: Bm25Params,
}

impl RelevanceScorer {
    /// Create a new scorer with keywords.
    #[must_use]
    pub fn new(query: &str, strategy: ScoringStrategyConfig) -> Self {
        let query_keywords = extract_keywords(query);
        Self {
            query_keywords,
            strategy,
            params: Bm25Params::default(),
        }
    }

    /// Create a scorer with pre-extracted keywords.
    #[must_use]
    pub fn with_keywords(keywords: Vec<String>, strategy: ScoringStrategyConfig) -> Self {
        Self {
            query_keywords: keywords,
            strategy,
            params: Bm25Params::default(),
        }
    }

    /// Score a content chunk.
    #[must_use]
    pub fn score_chunk(&self, chunk: &ContentChunk, ctx: &ScoringContext) -> ContentRelevance {
        let mut components = ScoreComponents::default();

        // 1. Keyword score (content + title + summary combined)
        components.keyword_score =
            self.compute_keyword_score(&format!("{} {}", chunk.title, chunk.content));

        // 2. BM25 score (if enabled)
        if matches!(
            self.strategy,
            ScoringStrategyConfig::KeywordWithBM25 | ScoringStrategyConfig::Hybrid
        ) {
            components.bm25_score = self.compute_bm25_score(&chunk.content, ctx);
        }

        // 3. Depth penalty (10% per level)
        components.depth_penalty = 0.9_f32.powi(chunk.depth as i32);

        // 4. Path bonus
        components.path_bonus = ctx.parent_score.map(|s| s * 0.2).unwrap_or(0.0);

        // 5. Density score
        components.density_score = compute_density(&chunk.content);

        let final_score = components.final_score();

        ContentRelevance::new(chunk.clone(), final_score, components)
    }

    /// Score multiple chunks.
    pub fn score_chunks<'a>(
        &self,
        chunks: &'a [ContentChunk],
        ctx: &ScoringContext,
    ) -> Vec<ContentRelevance> {
        chunks
            .iter()
            .map(|chunk| self.score_chunk(chunk, ctx))
            .collect()
    }

    /// Compute keyword overlap score.
    fn compute_keyword_score(&self, content: &str) -> f32 {
        if self.query_keywords.is_empty() {
            return 0.5; // Neutral score if no keywords
        }

        let content_lower = content.to_lowercase();
        let content_words: std::collections::HashSet<&str> =
            content_lower.split_whitespace().collect();

        let matches = self
            .query_keywords
            .iter()
            .filter(|kw| {
                let kw_lower = kw.to_lowercase();
                content_words.iter().any(|&w| w.contains(&kw_lower))
                    || content_lower.contains(&kw_lower)
            })
            .count();

        matches as f32 / self.query_keywords.len() as f32
    }

    /// Compute BM25 score.
    fn compute_bm25_score(&self, content: &str, ctx: &ScoringContext) -> f32 {
        if self.query_keywords.is_empty() {
            return 0.0;
        }

        let doc_len = content.split_whitespace().count() as f32;
        let mut score = 0.0;

        for term in &self.query_keywords {
            let term_lower = term.to_lowercase();
            let tf = content.to_lowercase().matches(&term_lower).count() as f32;

            if tf == 0.0 {
                continue;
            }

            // IDF calculation using BM25L variant
            let df = ctx.doc_freq.get(&term_lower).copied().unwrap_or(1) as f32;
            let idf = ((ctx.doc_count as f32 - df + 0.5) / (df + 0.5) + 1.0).ln();

            // BM25 formula
            let k1 = self.params.k1;
            let b = self.params.b;
            let numerator = tf * (k1 + 1.0);
            let denominator = tf + k1 * (1.0 - b + b * doc_len / ctx.avg_doc_len);

            score += idf * numerator / denominator;
        }

        // Normalize to [0, 1]
        let max_possible_score = self.query_keywords.len() as f32 * 5.0; // Rough upper bound
        (score / max_possible_score).clamp(0.0, 1.0)
    }

    /// Get the query keywords.
    #[must_use]
    pub fn keywords(&self) -> &[String] {
        &self.query_keywords
    }
}

/// Compute information density of content.
fn compute_density(content: &str) -> f32 {
    let words: Vec<&str> = content.split_whitespace().collect();
    if words.is_empty() {
        return 0.0;
    }

    // Use shared STOPWORDS from bm25 module
    let stopword_count = words
        .iter()
        .filter(|w| STOPWORDS.contains(&w.to_lowercase().as_str()))
        .count();

    let stopword_ratio = stopword_count as f32 / words.len() as f32;

    // Entity-like ratio (capitalized, numbers, special terms)
    let entity_count = words
        .iter()
        .filter(|w| w.chars().any(|c| c.is_numeric() || c.is_uppercase()))
        .count();

    let entity_ratio = entity_count as f32 / words.len() as f32;

    // Combined density score
    (1.0 - stopword_ratio) * 0.7 + entity_ratio * 0.3
}

#[cfg(test)]
mod tests {
    use super::*;
    use indextree::Arena;

    fn make_test_node_id() -> NodeId {
        let mut arena = Arena::new();
        let node = crate::document::TreeNode {
            title: "Test".to_string(),
            structure: String::new(),
            content: String::new(),
            summary: String::new(),
            depth: 0,
            start_index: 0,
            end_index: 0,
            start_page: None,
            end_page: None,
            node_id: None,
            physical_index: None,
            token_count: None,
            references: Vec::new(),
        };
        NodeId(arena.new_node(node))
    }

    #[test]
    fn test_keyword_extraction() {
        let keywords = extract_keywords("What is the architecture of vectorless?");
        assert!(keywords.contains(&"architecture".to_string()));
        assert!(keywords.contains(&"vectorless".to_string()));
        assert!(!keywords.contains(&"what".to_string())); // stopword
        assert!(!keywords.contains(&"the".to_string())); // stopword
    }

    #[test]
    fn test_density_score() {
        // High density content
        let high_density = "Rust 1.85+ requires Cargo.toml configuration with [dependencies]";
        let score = compute_density(high_density);
        assert!(score > 0.5);

        // Low density content (many stopwords)
        let low_density = "This is a test of the system with some words in it";
        let score = compute_density(low_density);
        assert!(score < 0.7);
    }

    #[test]
    fn test_depth_penalty() {
        let shallow = ContentChunk::new(
            make_test_node_id(),
            "Test".to_string(),
            "Content".to_string(),
            0,
        );

        let deep = ContentChunk::new(
            make_test_node_id(),
            "Test".to_string(),
            "Content".to_string(),
            5,
        );

        let scorer = RelevanceScorer::new("test", ScoringStrategyConfig::KeywordOnly);
        let ctx = ScoringContext::default();

        let shallow_score = scorer.score_chunk(&shallow, &ctx);
        let deep_score = scorer.score_chunk(&deep, &ctx);

        assert!(shallow_score.components.depth_penalty > deep_score.components.depth_penalty);
    }

    #[test]
    fn test_score_components_final_score() {
        let components = ScoreComponents {
            keyword_score: 0.8,
            bm25_score: 0.6,
            depth_penalty: 0.9,
            path_bonus: 0.1,
            density_score: 0.5,
        };

        let final_score = components.final_score();
        assert!(final_score > 0.0 && final_score <= 1.0);
    }
}
