// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Main content aggregator combining all components.
//!
//! This module provides the main [`ContentAggregator`] that orchestrates
//! scoring, budget allocation, and structure building.

use std::collections::HashMap;

use tracing::{debug, info};

use crate::document::{DocumentTree, NodeId};
use crate::utils::estimate_tokens;

use super::budget::{AllocationStrategy, BudgetAllocator};
use super::builder::{ContentMetadata, StructureBuilder};
use super::config::ContentAggregatorConfig;
use super::scorer::{ContentChunk, RelevanceScorer, ScoringContext};

/// Candidate node from retrieval.
#[derive(Debug, Clone)]
pub struct CandidateNode {
    /// Node ID.
    pub node_id: NodeId,
    /// Relevance score from search.
    pub score: f32,
    /// Depth in tree.
    pub depth: usize,
}

impl CandidateNode {
    /// Create a new candidate.
    #[must_use]
    pub fn new(node_id: NodeId, score: f32, depth: usize) -> Self {
        Self {
            node_id,
            score,
            depth,
        }
    }
}

/// Result of content aggregation.
#[derive(Debug, Clone)]
pub struct AggregationResult {
    /// Aggregated content string.
    pub content: String,
    /// Total tokens used.
    pub tokens_used: usize,
    /// Number of nodes included.
    pub nodes_included: usize,
    /// Average relevance score.
    pub avg_score: f32,
    /// Whether content was truncated due to budget.
    pub was_truncated: bool,
    /// Metadata about the aggregation.
    pub metadata: ContentMetadata,
}

impl AggregationResult {
    /// Check if result is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

/// Content aggregator combining scoring, allocation, and building.
#[derive(Debug)]
pub struct ContentAggregator {
    /// Configuration.
    config: ContentAggregatorConfig,
}

impl ContentAggregator {
    /// Create a new content aggregator.
    #[must_use]
    pub fn new(config: ContentAggregatorConfig) -> Self {
        Self { config }
    }

    /// Create aggregator with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(ContentAggregatorConfig::default())
    }

    /// Aggregate content from candidate nodes.
    ///
    /// # Arguments
    ///
    /// * `candidates` - Candidate nodes from retrieval
    /// * `tree` - Document tree
    /// * `query` - Query string for relevance scoring
    ///
    /// # Returns
    ///
    /// Aggregated content within token budget.
    #[must_use]
    pub fn aggregate(
        &self,
        candidates: &[CandidateNode],
        tree: &DocumentTree,
        query: &str,
    ) -> AggregationResult {
        let start = std::time::Instant::now();

        // Step 1: Collect all content chunks from candidates and their descendants
        let chunks = self.collect_chunks(candidates, tree);
        debug!(
            "Collected {} content chunks from {} candidates",
            chunks.len(),
            candidates.len()
        );

        if chunks.is_empty() {
            return AggregationResult {
                content: String::new(),
                tokens_used: 0,
                nodes_included: 0,
                avg_score: 0.0,
                was_truncated: false,
                metadata: ContentMetadata::default(),
            };
        }

        // Step 2: Score all chunks for relevance
        let scorer = RelevanceScorer::new(query, self.config.scoring_strategy);
        let scoring_ctx = self.build_scoring_context(&chunks);
        let scored = scorer.score_chunks(&chunks, &scoring_ctx);

        // Filter by minimum score
        let filtered: Vec<_> = scored
            .into_iter()
            .filter(|r| r.score >= self.config.min_relevance_score)
            .collect();

        debug!(
            "Scored {} chunks, {} passed threshold {:.2}",
            chunks.len(),
            filtered.len(),
            self.config.min_relevance_score
        );

        if filtered.is_empty() {
            // Fall back to returning best candidate content
            return self.fallback_result(candidates, tree);
        }

        // Step 3: Allocate token budget
        let max_depth = filtered.iter().map(|r| r.chunk.depth).max().unwrap_or(0);
        let strategy = self.get_allocation_strategy();
        let allocator = BudgetAllocator::new(self.config.token_budget).with_strategy(strategy);

        let allocation = allocator.allocate(filtered, max_depth);

        info!(
            "Allocated {} tokens to {} items (strategy: {:?})",
            allocation.tokens_used,
            allocation.selected.len(),
            self.config.scoring_strategy
        );

        // Step 4: Build structured output
        let builder =
            StructureBuilder::from_config(self.config.output_format, self.config.include_scores);

        let structured = builder.build(allocation.selected.clone(), tree);

        // Build result
        let was_truncated = allocation.selected.iter().any(|s| s.is_truncated());

        AggregationResult {
            content: structured.content,
            tokens_used: allocation.tokens_used,
            nodes_included: allocation.selected.len(),
            avg_score: allocation.stats.avg_score,
            was_truncated,
            metadata: structured.metadata,
        }
    }

    /// Collect content chunks from candidates and descendants.
    fn collect_chunks(
        &self,
        candidates: &[CandidateNode],
        tree: &DocumentTree,
    ) -> Vec<ContentChunk> {
        let mut chunks = Vec::new();
        let mut visited: HashMap<NodeId, bool> = HashMap::new();

        for candidate in candidates {
            // Add candidate's own content
            if let Some(node) = tree.get(candidate.node_id) {
                if !node.content.is_empty() {
                    chunks.push(ContentChunk::new(
                        candidate.node_id,
                        node.title.clone(),
                        node.content.clone(),
                        candidate.depth,
                    ));
                    visited.insert(candidate.node_id, true);
                }

                // Collect leaf descendants
                self.collect_descendant_chunks(
                    candidate.node_id,
                    tree,
                    candidate.depth,
                    &mut chunks,
                    &mut visited,
                );
            }
        }

        chunks
    }

    /// Collect chunks from descendant nodes.
    fn collect_descendant_chunks(
        &self,
        parent_id: NodeId,
        tree: &DocumentTree,
        parent_depth: usize,
        chunks: &mut Vec<ContentChunk>,
        visited: &mut HashMap<NodeId, bool>,
    ) {
        let children = tree.children(parent_id);

        for child_id in children {
            if visited.contains_key(&child_id) {
                continue;
            }
            visited.insert(child_id, true);

            if let Some(node) = tree.get(child_id) {
                let child_depth = parent_depth + 1;

                if tree.is_leaf(child_id) {
                    // Leaf node - add its content
                    if !node.content.is_empty() {
                        chunks.push(ContentChunk::new(
                            child_id,
                            node.title.clone(),
                            node.content.clone(),
                            child_depth,
                        ));
                    }
                } else {
                    // Non-leaf - recurse
                    self.collect_descendant_chunks(child_id, tree, child_depth, chunks, visited);
                }
            }
        }
    }

    /// Build scoring context from chunks.
    fn build_scoring_context(&self, chunks: &[ContentChunk]) -> ScoringContext {
        let total_len: usize = chunks.iter().map(|c| c.content.len()).sum();
        let avg_len = if chunks.is_empty() {
            100.0
        } else {
            total_len as f32 / chunks.len() as f32
        };

        // Build document frequency map
        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        for chunk in chunks {
            let mut seen_in_doc = std::collections::HashSet::new();
            for word in chunk.content.to_lowercase().split_whitespace() {
                if !seen_in_doc.contains(word) {
                    *doc_freq.entry(word.to_string()).or_insert(0) += 1;
                    seen_in_doc.insert(word);
                }
            }
        }

        ScoringContext {
            avg_doc_len: avg_len,
            doc_count: chunks.len(),
            doc_freq,
            parent_score: None,
        }
    }

    /// Get allocation strategy from config.
    fn get_allocation_strategy(&self) -> AllocationStrategy {
        AllocationStrategy::Hierarchical {
            min_per_level: self.config.hierarchical_min_per_level,
        }
    }

    /// Fallback result when no content passes threshold.
    fn fallback_result(
        &self,
        candidates: &[CandidateNode],
        tree: &DocumentTree,
    ) -> AggregationResult {
        // Return best candidate's content
        if let Some(best) = candidates.first() {
            if let Some(node) = tree.get(best.node_id) {
                let content = if !node.content.is_empty() {
                    node.content.clone()
                } else if !node.summary.is_empty() {
                    node.summary.clone()
                } else {
                    String::new()
                };

                let tokens = estimate_tokens(&content);

                return AggregationResult {
                    content: format!("## {}\n\n{}", node.title, content),
                    tokens_used: tokens,
                    nodes_included: 1,
                    avg_score: best.score,
                    was_truncated: false,
                    metadata: ContentMetadata {
                        total_tokens: tokens,
                        node_count: 1,
                        avg_score: best.score,
                        max_depth: best.depth,
                    },
                };
            }
        }

        AggregationResult {
            content: String::new(),
            tokens_used: 0,
            nodes_included: 0,
            avg_score: 0.0,
            was_truncated: false,
            metadata: ContentMetadata::default(),
        }
    }
}

impl Default for ContentAggregator {
    fn default() -> Self {
        Self::with_defaults()
    }
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
    fn test_aggregator_creation() {
        let config = ContentAggregatorConfig::default();
        let aggregator = ContentAggregator::new(config);
        assert_eq!(aggregator.config.token_budget, 4000);
    }

    #[test]
    fn test_aggregator_with_defaults() {
        let aggregator = ContentAggregator::with_defaults();
        assert_eq!(aggregator.config.token_budget, 4000);
    }

    #[test]
    fn test_empty_candidates() {
        let aggregator = ContentAggregator::with_defaults();
        let tree = DocumentTree::new("Test", "");

        let result = aggregator.aggregate(&[], &tree, "test query");

        assert!(result.is_empty());
        assert_eq!(result.tokens_used, 0);
    }

    #[test]
    fn test_candidate_node_creation() {
        let node_id = make_test_node_id();
        let candidate = CandidateNode::new(node_id, 0.8, 2);

        assert_eq!(candidate.score, 0.8);
        assert_eq!(candidate.depth, 2);
    }
}
