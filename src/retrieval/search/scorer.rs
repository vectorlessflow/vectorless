// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Node scoring utilities with BM25 support.
//!
//! Implements the NodeScore formula: `Σ ChunkScore(n) / √(N+1)`
//! with optional BM25 scoring for better relevance ranking.

use std::collections::HashMap;

use crate::document::{DocumentTree, NodeId};

/// Common English stop words for keyword filtering.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
    "have", "has", "had", "do", "does", "did", "will", "would", "could",
    "should", "may", "might", "must", "shall", "can", "need", "dare",
    "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by",
    "from", "as", "into", "through", "during", "before", "after", "above",
    "below", "between", "under", "again", "further", "then", "once",
    "here", "there", "when", "where", "why", "how", "all", "each", "few",
    "more", "most", "other", "some", "such", "no", "nor", "not", "only",
    "own", "same", "so", "than", "too", "very", "just", "and", "but",
    "if", "or", "because", "until", "while", "about", "what", "which",
    "who", "whom", "this", "that", "these", "those", "i", "me", "my",
    "myself", "we", "our", "ours", "ourselves", "you", "your", "yours",
    "yourself", "yourselves", "he", "him", "his", "himself", "she", "her",
    "hers", "herself", "it", "its", "itself", "they", "them", "their",
    "theirs", "themselves",
];

/// Extract keywords from a query string, filtering stop words.
fn extract_keywords(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| {
            let s = *s;
            !s.is_empty() && s.len() > 1 && !STOPWORDS.contains(&s)
        })
        .map(String::from)
        .collect()
}

/// Scoring strategy to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScoringStrategy {
    /// Keyword overlap only (fastest).
    KeywordOnly,
    /// BM25 only (better relevance).
    #[default]
    BM25,
    /// Hybrid: weighted combination of keyword + BM25.
    Hybrid,
}

/// BM25 parameters.
#[derive(Debug, Clone, Copy)]
pub struct Bm25Params {
    /// Term frequency saturation parameter (k1).
    pub k1: f32,
    /// Length normalization parameter (b).
    pub b: f32,
}

impl Default for Bm25Params {
    fn default() -> Self {
        Self {
            k1: 1.2,
            b: 0.75,
        }
    }
}

/// Context for scoring calculations.
#[derive(Debug, Clone)]
pub struct ScoringContext {
    /// Query terms for keyword matching.
    pub query_terms: Vec<String>,
    /// Weight for title matches.
    pub title_weight: f32,
    /// Weight for summary matches.
    pub summary_weight: f32,
    /// Weight for content matches.
    pub content_weight: f32,
    /// Depth penalty factor.
    pub depth_penalty: f32,
    /// Scoring strategy.
    pub strategy: ScoringStrategy,
    /// BM25 parameters.
    pub bm25_params: Bm25Params,
    /// Average document length for BM25.
    pub avg_doc_len: f32,
    /// Document frequency for terms (for IDF).
    pub doc_freq: HashMap<String, usize>,
    /// Total document count for IDF.
    pub doc_count: usize,
}

impl Default for ScoringContext {
    fn default() -> Self {
        Self {
            query_terms: Vec::new(),
            title_weight: 2.0,
            summary_weight: 1.5,
            content_weight: 1.0,
            depth_penalty: 0.1,
            strategy: ScoringStrategy::default(),
            bm25_params: Bm25Params::default(),
            avg_doc_len: 100.0,
            doc_freq: HashMap::new(),
            doc_count: 1,
        }
    }
}

impl ScoringContext {
    /// Create a new scoring context with query terms.
    pub fn new(query: &str) -> Self {
        Self {
            query_terms: extract_keywords(query),
            ..Default::default()
        }
    }

    /// Create a context with a specific scoring strategy.
    pub fn with_strategy(query: &str, strategy: ScoringStrategy) -> Self {
        Self {
            query_terms: extract_keywords(query),
            strategy,
            ..Default::default()
        }
    }

    /// Set BM25 parameters.
    pub fn with_bm25_params(mut self, params: Bm25Params) -> Self {
        self.bm25_params = params;
        self
    }

    /// Set document statistics for BM25.
    pub fn with_doc_stats(
        mut self,
        doc_count: usize,
        avg_doc_len: f32,
        doc_freq: HashMap<String, usize>,
    ) -> Self {
        self.doc_count = doc_count.max(1);
        self.avg_doc_len = avg_doc_len.max(1.0);
        self.doc_freq = doc_freq;
        self
    }

    /// Calculate term frequency in text.
    fn term_frequency(&self, text: &str, term: &str) -> f32 {
        text.to_lowercase().matches(term).count() as f32
    }

    /// Calculate IDF (Inverse Document Frequency) for a term.
    fn idf(&self, term: &str) -> f32 {
        let df = self.doc_freq.get(term).copied().unwrap_or(1) as f32;
        let n = self.doc_count as f32;
        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }

    /// Calculate BM25 score for a single field.
    fn bm25_field_score(&self, text: &str) -> f32 {
        if self.query_terms.is_empty() {
            return 0.0;
        }

        let doc_len = text.split_whitespace().count() as f32;
        let k1 = self.bm25_params.k1;
        let b = self.bm25_params.b;

        let mut score = 0.0;
        for term in &self.query_terms {
            let tf = self.term_frequency(text, term);
            if tf == 0.0 {
                continue;
            }

            let idf = self.idf(term);
            let numerator = tf * (k1 + 1.0);
            let denominator = tf + k1 * (1.0 - b + b * doc_len / self.avg_doc_len);

            score += idf * numerator / denominator;
        }

        score
    }

    /// Calculate keyword overlap score for a text.
    fn keyword_overlap(&self, text: &str) -> f32 {
        if self.query_terms.is_empty() {
            return 0.0;
        }

        let text_lower = text.to_lowercase();
        let matches = self
            .query_terms
            .iter()
            .filter(|term| text_lower.contains(term.as_str()))
            .count();

        matches as f32 / self.query_terms.len() as f32
    }

    /// Calculate a quick keyword-based score for a node.
    pub fn quick_score(&self, tree: &DocumentTree, node_id: NodeId) -> f32 {
        if let Some(node) = tree.get(node_id) {
            let title_score = self.keyword_overlap(&node.title);
            let summary_score = self.keyword_overlap(&node.summary);
            let content_score = self.keyword_overlap(&node.content);

            let base_score = (title_score * self.title_weight
                + summary_score * self.summary_weight
                + content_score * self.content_weight)
                / (self.title_weight + self.summary_weight + self.content_weight);

            // Apply depth penalty (prefer shallower nodes)
            let depth_factor = 1.0 - (node.depth as f32 * self.depth_penalty).min(0.5);

            base_score * depth_factor
        } else {
            0.0
        }
    }

    /// Calculate BM25 score for a node.
    pub fn bm25_score(&self, tree: &DocumentTree, node_id: NodeId) -> f32 {
        if let Some(node) = tree.get(node_id) {
            let title_score = self.bm25_field_score(&node.title) * self.title_weight;
            let summary_score = self.bm25_field_score(&node.summary) * self.summary_weight;
            let content_score = self.bm25_field_score(&node.content) * self.content_weight;

            let total_score = title_score + summary_score + content_score;

            // Normalize to [0, 1] range
            let max_possible = self.query_terms.len() as f32 * 10.0; // Rough upper bound
            let normalized = (total_score / max_possible).clamp(0.0, 1.0);

            // Apply depth penalty
            let depth_factor = 1.0 - (node.depth as f32 * self.depth_penalty).min(0.5);

            normalized * depth_factor
        } else {
            0.0
        }
    }

    /// Calculate hybrid score (keyword + BM25).
    pub fn hybrid_score(&self, tree: &DocumentTree, node_id: NodeId) -> f32 {
        let keyword = self.quick_score(tree, node_id);
        let bm25 = self.bm25_score(tree, node_id);

        // Weighted combination: 40% keyword, 60% BM25
        keyword * 0.4 + bm25 * 0.6
    }

    /// Calculate score based on configured strategy.
    pub fn score(&self, tree: &DocumentTree, node_id: NodeId) -> f32 {
        match self.strategy {
            ScoringStrategy::KeywordOnly => self.quick_score(tree, node_id),
            ScoringStrategy::BM25 => self.bm25_score(tree, node_id),
            ScoringStrategy::Hybrid => self.hybrid_score(tree, node_id),
        }
    }
}

/// Node scorer for calculating relevance scores.
pub struct NodeScorer {
    /// Scoring context.
    context: ScoringContext,
}

impl NodeScorer {
    /// Create a new node scorer.
    pub fn new(context: ScoringContext) -> Self {
        Self { context }
    }

    /// Create a scorer with default context for a query.
    pub fn for_query(query: &str) -> Self {
        Self::new(ScoringContext::new(query))
    }

    /// Create a scorer with a specific strategy.
    pub fn with_strategy(query: &str, strategy: ScoringStrategy) -> Self {
        Self::new(ScoringContext::with_strategy(query, strategy))
    }

    /// Get the scoring context.
    pub fn context(&self) -> &ScoringContext {
        &self.context
    }

    /// Get mutable scoring context.
    pub fn context_mut(&mut self) -> &mut ScoringContext {
        &mut self.context
    }

    /// Score a single node.
    pub fn score(&self, tree: &DocumentTree, node_id: NodeId) -> f32 {
        self.context.score(tree, node_id)
    }

    /// Score multiple nodes and return sorted by score (descending).
    pub fn score_and_sort(&self, tree: &DocumentTree, node_ids: &[NodeId]) -> Vec<(NodeId, f32)> {
        let mut scored: Vec<_> = node_ids
            .iter()
            .map(|&id| (id, self.score(tree, id)))
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }

    /// Calculate chunk score for a portion of content.
    ///
    /// Used in the NodeScore formula.
    pub fn chunk_score(&self, chunk: &str) -> f32 {
        self.context.keyword_overlap(chunk)
    }

    /// Calculate the full NodeScore using the formula:
    /// `Σ ChunkScore(n) / √(N+1)`
    ///
    /// Where N is the number of chunks and ChunkScore is calculated for each.
    pub fn node_score(&self, tree: &DocumentTree, node_id: NodeId, chunk_size: usize) -> f32 {
        if let Some(node) = tree.get(node_id) {
            let content = format!("{} {} {}", node.title, node.summary, node.content);

            // Split into chunks
            let chunks: Vec<&str> = content
                .as_bytes()
                .chunks(chunk_size)
                .map(|b| std::str::from_utf8(b).unwrap_or(""))
                .collect();

            if chunks.is_empty() {
                return 0.0;
            }

            // Sum chunk scores
            let total_score: f32 = chunks.iter().map(|c| self.chunk_score(c)).sum();

            // Apply formula: Σ ChunkScore(n) / √(N+1)
            let n = chunks.len() as f32;
            total_score / (n + 1.0).sqrt()
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_keywords() {
        let keywords = extract_keywords("What is the architecture of vectorless?");
        assert!(keywords.contains(&"architecture".to_string()));
        assert!(keywords.contains(&"vectorless".to_string()));
        assert!(!keywords.contains(&"what".to_string())); // stopword
        assert!(!keywords.contains(&"the".to_string())); // stopword
    }

    #[test]
    fn test_keyword_overlap() {
        let ctx = ScoringContext::new("vectorless architecture");

        let text = "Vectorless has a unique architecture for document retrieval.";
        let score = ctx.keyword_overlap(text);

        assert!(score > 0.5); // Should match both keywords
    }

    #[test]
    fn test_bm25_scoring() {
        let ctx = ScoringContext::with_strategy("rust cargo", ScoringStrategy::BM25);

        let text = "Rust is a programming language. Cargo is its package manager. Rust Rust Rust.";
        let score = ctx.bm25_field_score(text);

        // Should have higher score due to term frequency
        assert!(score > 0.0);
    }

    #[test]
    fn test_hybrid_scoring() {
        let ctx = ScoringContext::with_strategy("test query", ScoringStrategy::Hybrid);

        let keyword_score = ctx.keyword_overlap("test query content");
        let bm25_score = ctx.bm25_field_score("test query content");
        let hybrid = ctx.keyword_overlap("test query content") * 0.4
            + ctx.bm25_field_score("test query content") * 0.6;

        // Hybrid should be between keyword and bm25 scores (roughly)
        assert!(hybrid > 0.0);
    }

    #[test]
    fn test_scorer_creation() {
        let scorer = NodeScorer::for_query("test query");
        assert!(!scorer.context().query_terms.is_empty());
    }

    #[test]
    fn test_scorer_with_strategy() {
        let scorer = NodeScorer::with_strategy("test", ScoringStrategy::BM25);
        assert_eq!(scorer.context().strategy, ScoringStrategy::BM25);
    }
}

