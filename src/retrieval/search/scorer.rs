// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Node scoring utilities.
//!
//! Implements the NodeScore formula: `Σ ChunkScore(n) / √(N+1)`

use crate::domain::{NodeId, DocumentTree};

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
}

impl Default for ScoringContext {
    fn default() -> Self {
        Self {
            query_terms: Vec::new(),
            title_weight: 2.0,
            summary_weight: 1.5,
            content_weight: 1.0,
            depth_penalty: 0.1,
        }
    }
}

impl ScoringContext {
    /// Create a new scoring context with query terms.
    pub fn new(query: &str) -> Self {
        Self {
            query_terms: query
                .to_lowercase()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect(),
            ..Default::default()
        }
    }

    /// Calculate a quick keyword-based score for a node.
    pub fn quick_score(&self, tree: &DocumentTree, node_id: NodeId) -> f32 {
        if let Some(node) = tree.get(node_id) {
            let title_score = self.term_overlap(&node.title);
            let summary_score = self.term_overlap(&node.summary);
            let content_score = self.term_overlap(&node.content);

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

    /// Calculate term overlap between query and text.
    fn term_overlap(&self, text: &str) -> f32 {
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

    /// Score a single node.
    pub fn score(&self, tree: &DocumentTree, node_id: NodeId) -> f32 {
        self.context.quick_score(tree, node_id)
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
        self.context.term_overlap(chunk)
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
