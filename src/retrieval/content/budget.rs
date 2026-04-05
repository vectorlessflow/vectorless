// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Token budget allocation for content aggregation.
//!
//! This module provides budget-aware content selection that optimizes
//! token usage while maximizing relevance.

use std::collections::HashMap;

use crate::document::NodeId;
use crate::util::estimate_tokens;

use super::scorer::ContentRelevance;

/// Allocation strategy for distributing token budget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AllocationStrategy {
    /// Select highest-scoring content first until budget exhausted.
    Greedy,
    /// Distribute budget proportionally to relevance scores.
    Proportional,
    /// Ensure each depth level has minimum representation.
    Hierarchical {
        /// Minimum fraction of budget per level (0.0 - 1.0)
        min_per_level: f32,
    },
}

impl Default for AllocationStrategy {
    fn default() -> Self {
        Self::Hierarchical { min_per_level: 0.1 }
    }
}

/// Information about content truncation.
#[derive(Debug, Clone)]
pub struct TruncationInfo {
    /// Original content length in characters.
    pub original_len: usize,
    /// Truncated content length in characters.
    pub truncated_len: usize,
    /// Reason for truncation.
    pub reason: TruncationReason,
}

/// Reason for content truncation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncationReason {
    /// Content exceeded remaining budget.
    BudgetExceeded,
    /// Content tail had low relevance.
    LowRelevanceTail,
}

/// A selected content item after budget allocation.
#[derive(Debug, Clone)]
pub struct SelectedContent {
    /// Node ID.
    pub node_id: NodeId,
    /// Node title.
    pub title: String,
    /// Selected content text.
    pub content: String,
    /// Token count of selected content.
    pub tokens: usize,
    /// Relevance score.
    pub score: f32,
    /// Depth in tree.
    pub depth: usize,
    /// Truncation info if content was truncated.
    pub truncation: Option<TruncationInfo>,
}

impl SelectedContent {
    /// Check if content was truncated.
    #[must_use]
    pub fn is_truncated(&self) -> bool {
        self.truncation.is_some()
    }
}

/// Statistics about the allocation process.
#[derive(Debug, Clone, Default)]
pub struct AllocationStats {
    /// Total content items considered.
    pub items_considered: usize,
    /// Items selected for output.
    pub items_selected: usize,
    /// Items truncated.
    pub items_truncated: usize,
    /// Items filtered (below threshold).
    pub items_filtered: usize,
    /// Average score of selected items.
    pub avg_score: f32,
}

/// Result of budget allocation.
#[derive(Debug, Clone)]
pub struct AllocationResult {
    /// Selected content items.
    pub selected: Vec<SelectedContent>,
    /// Total tokens used.
    pub tokens_used: usize,
    /// Remaining token budget.
    pub remaining_budget: usize,
    /// Allocation statistics.
    pub stats: AllocationStats,
}

impl AllocationResult {
    /// Check if any content was selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Get number of selected items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.selected.len()
    }
}

/// Token budget allocator.
#[derive(Debug)]
pub struct BudgetAllocator {
    /// Total token budget.
    total_budget: usize,
    /// Minimum reserve budget (for fallback).
    min_reserve: usize,
    /// Allocation strategy.
    strategy: AllocationStrategy,
    /// Minimum relevance score threshold.
    min_score: f32,
}

impl BudgetAllocator {
    /// Create a new allocator with the specified budget.
    #[must_use]
    pub fn new(budget: usize) -> Self {
        Self {
            total_budget: budget,
            min_reserve: budget / 10,
            strategy: AllocationStrategy::default(),
            min_score: 0.0,
        }
    }

    /// Set the allocation strategy.
    #[must_use]
    pub fn with_strategy(mut self, strategy: AllocationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set minimum relevance score threshold.
    #[must_use]
    pub fn with_min_score(mut self, min_score: f32) -> Self {
        self.min_score = min_score;
        self
    }

    /// Allocate budget to scored content.
    #[must_use]
    pub fn allocate(
        &self,
        scored_content: Vec<ContentRelevance>,
        max_depth: usize,
    ) -> AllocationResult {
        // Filter by minimum score
        let filtered: Vec<_> = scored_content
            .into_iter()
            .filter(|c| c.score >= self.min_score)
            .collect();

        let stats = AllocationStats {
            items_considered: filtered.len(),
            ..Default::default()
        };

        match &self.strategy {
            AllocationStrategy::Greedy => self.allocate_greedy(filtered, stats),
            AllocationStrategy::Proportional => self.allocate_proportional(filtered, stats),
            AllocationStrategy::Hierarchical { min_per_level } => {
                self.allocate_hierarchical(filtered, max_depth, *min_per_level, stats)
            }
        }
    }

    /// Greedy allocation: select highest-scoring content first.
    fn allocate_greedy(
        &self,
        mut content: Vec<ContentRelevance>,
        mut stats: AllocationStats,
    ) -> AllocationResult {
        // Sort by score descending
        content.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut selected = Vec::new();
        let mut tokens_used = 0;

        for relevance in content {
            let tokens = relevance.chunk.token_count();

            if tokens_used + tokens <= self.total_budget {
                selected.push(SelectedContent {
                    node_id: relevance.chunk.node_id,
                    title: relevance.chunk.title,
                    content: relevance.chunk.content,
                    tokens,
                    score: relevance.score,
                    depth: relevance.chunk.depth,
                    truncation: None,
                });
                tokens_used += tokens;
            } else {
                // Try to fit truncated content
                let remaining = self.total_budget - tokens_used;
                if remaining >= 50 {
                    // Minimum useful content
                    if let Some(truncated) =
                        self.truncate_content(&relevance.chunk.content, remaining)
                    {
                        let truncated_tokens = estimate_tokens(&truncated);
                        selected.push(SelectedContent {
                            node_id: relevance.chunk.node_id,
                            title: relevance.chunk.title,
                            content: truncated,
                            tokens: truncated_tokens,
                            score: relevance.score,
                            depth: relevance.chunk.depth,
                            truncation: Some(TruncationInfo {
                                original_len: relevance.chunk.content.len(),
                                truncated_len: remaining,
                                reason: TruncationReason::BudgetExceeded,
                            }),
                        });
                        tokens_used += truncated_tokens;
                        stats.items_truncated += 1;
                    }
                }
                break;
            }
        }

        stats.items_selected = selected.len();
        stats.avg_score = if selected.is_empty() {
            0.0
        } else {
            selected.iter().map(|s| s.score).sum::<f32>() / selected.len() as f32
        };

        AllocationResult {
            selected,
            tokens_used,
            remaining_budget: self.total_budget - tokens_used,
            stats,
        }
    }

    /// Proportional allocation: distribute budget by score ratio.
    fn allocate_proportional(
        &self,
        content: Vec<ContentRelevance>,
        mut stats: AllocationStats,
    ) -> AllocationResult {
        let total_score: f32 = content.iter().map(|c| c.score).sum();
        if total_score == 0.0 {
            return AllocationResult {
                selected: Vec::new(),
                tokens_used: 0,
                remaining_budget: self.total_budget,
                stats,
            };
        }

        let mut selected = Vec::new();
        let mut tokens_used = 0;

        for relevance in content {
            // Calculate proportional budget
            let proportion = relevance.score / total_score;
            let allocated_budget = ((self.total_budget as f32 * proportion) as usize).max(50);

            let content_tokens = relevance.chunk.token_count();

            if content_tokens <= allocated_budget {
                // Full content fits
                if tokens_used + content_tokens <= self.total_budget {
                    selected.push(SelectedContent {
                        node_id: relevance.chunk.node_id,
                        title: relevance.chunk.title,
                        content: relevance.chunk.content,
                        tokens: content_tokens,
                        score: relevance.score,
                        depth: relevance.chunk.depth,
                        truncation: None,
                    });
                    tokens_used += content_tokens;
                }
            } else {
                // Truncate to allocated budget
                let remaining = self.total_budget - tokens_used;
                if remaining >= 50 && remaining >= allocated_budget / 2 {
                    if let Some(truncated) = self
                        .truncate_content(&relevance.chunk.content, remaining.min(allocated_budget))
                    {
                        let truncated_tokens = estimate_tokens(&truncated);
                        let truncated_len = truncated.len();
                        selected.push(SelectedContent {
                            node_id: relevance.chunk.node_id,
                            title: relevance.chunk.title,
                            content: truncated,
                            tokens: truncated_tokens,
                            score: relevance.score,
                            depth: relevance.chunk.depth,
                            truncation: Some(TruncationInfo {
                                original_len: relevance.chunk.content.len(),
                                truncated_len,
                                reason: TruncationReason::BudgetExceeded,
                            }),
                        });
                        tokens_used += truncated_tokens;
                        stats.items_truncated += 1;
                    }
                }
            }
        }

        stats.items_selected = selected.len();
        stats.avg_score = if selected.is_empty() {
            0.0
        } else {
            selected.iter().map(|s| s.score).sum::<f32>() / selected.len() as f32
        };

        AllocationResult {
            selected,
            tokens_used,
            remaining_budget: self.total_budget - tokens_used,
            stats,
        }
    }

    /// Hierarchical allocation: ensure each depth level has representation.
    fn allocate_hierarchical(
        &self,
        content: Vec<ContentRelevance>,
        max_depth: usize,
        min_per_level: f32,
        mut stats: AllocationStats,
    ) -> AllocationResult {
        // Group content by depth
        let mut by_depth: HashMap<usize, Vec<ContentRelevance>> = HashMap::new();
        for c in content {
            by_depth.entry(c.chunk.depth).or_default().push(c);
        }

        // Sort each level by score
        for (_depth, items) in by_depth.iter_mut() {
            items.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        let per_level_budget = (self.total_budget as f32 * min_per_level) as usize;
        let mut selected = Vec::new();
        let mut tokens_used = 0;

        // Process from shallow to deep
        for depth in 0..=max_depth {
            if tokens_used >= self.total_budget {
                break;
            }

            if let Some(level_content) = by_depth.get(&depth) {
                let mut level_used = 0;

                for relevance in level_content {
                    if tokens_used >= self.total_budget {
                        break;
                    }

                    let tokens = relevance.chunk.token_count();

                    // Check if we should include this content
                    let can_include_full = tokens_used + tokens <= self.total_budget;
                    let level_budget_ok = level_used < per_level_budget || depth == 0;

                    if can_include_full && level_budget_ok {
                        selected.push(SelectedContent {
                            node_id: relevance.chunk.node_id,
                            title: relevance.chunk.title.clone(),
                            content: relevance.chunk.content.clone(),
                            tokens,
                            score: relevance.score,
                            depth,
                            truncation: None,
                        });
                        tokens_used += tokens;
                        level_used += tokens;
                    } else if level_used < per_level_budget {
                        // Try truncated version
                        let remaining =
                            (self.total_budget - tokens_used).min(per_level_budget - level_used);
                        if remaining >= 50 {
                            if let Some(truncated) =
                                self.truncate_content(&relevance.chunk.content, remaining)
                            {
                                let truncated_tokens = estimate_tokens(&truncated);
                                selected.push(SelectedContent {
                                    node_id: relevance.chunk.node_id,
                                    title: relevance.chunk.title.clone(),
                                    content: truncated,
                                    tokens: truncated_tokens,
                                    score: relevance.score,
                                    depth,
                                    truncation: Some(TruncationInfo {
                                        original_len: relevance.chunk.content.len(),
                                        truncated_len: remaining,
                                        reason: TruncationReason::BudgetExceeded,
                                    }),
                                });
                                tokens_used += truncated_tokens;
                                level_used += truncated_tokens;
                                stats.items_truncated += 1;
                            }
                        }
                    }
                }
            }
        }

        // Second pass: fill remaining budget with highest-scoring content
        if tokens_used < self.total_budget - self.min_reserve {
            let mut all_remaining: Vec<_> = by_depth
                .values()
                .flat_map(|v| v.iter())
                .filter(|c| !selected.iter().any(|s| s.node_id == c.chunk.node_id))
                .collect();

            all_remaining.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            for relevance in all_remaining {
                if tokens_used >= self.total_budget - self.min_reserve {
                    break;
                }

                let tokens = relevance.chunk.token_count();
                if tokens_used + tokens <= self.total_budget {
                    selected.push(SelectedContent {
                        node_id: relevance.chunk.node_id,
                        title: relevance.chunk.title.clone(),
                        content: relevance.chunk.content.clone(),
                        tokens,
                        score: relevance.score,
                        depth: relevance.chunk.depth,
                        truncation: None,
                    });
                    tokens_used += tokens;
                }
            }
        }

        stats.items_selected = selected.len();
        stats.avg_score = if selected.is_empty() {
            0.0
        } else {
            selected.iter().map(|s| s.score).sum::<f32>() / selected.len() as f32
        };

        AllocationResult {
            selected,
            tokens_used,
            remaining_budget: self.total_budget - tokens_used,
            stats,
        }
    }

    /// Truncate content to fit within token budget.
    fn truncate_content(&self, content: &str, max_tokens: usize) -> Option<String> {
        if max_tokens < 20 {
            return None;
        }

        // Approximate: 1 token ≈ 4 characters (for English)
        let max_chars = max_tokens * 4;

        if content.len() <= max_chars {
            return Some(content.to_string());
        }

        // Try to break at sentence boundary
        let truncated = &content[..max_chars];

        // Find last sentence boundary
        if let Some(pos) = truncated.rfind(|c| c == '.' || c == '!' || c == '?') {
            Some(format!("{}...", &truncated[..=pos]))
        } else if let Some(pos) = truncated.rfind(' ') {
            // Fall back to word boundary
            Some(format!("{}...", &truncated[..pos]))
        } else {
            // Hard truncate
            Some(format!("{}...", truncated))
        }
    }
}

impl Default for BudgetAllocator {
    fn default() -> Self {
        Self::new(4000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::content::{ContentChunk, ScoreComponents};
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

    fn make_relevance(content: &str, score: f32, depth: usize) -> ContentRelevance {
        let chunk = ContentChunk::new(
            make_test_node_id(),
            "Test".to_string(),
            content.to_string(),
            depth,
        );
        ContentRelevance::new(chunk, score, ScoreComponents::default())
    }

    #[test]
    fn test_allocator_creation() {
        let allocator = BudgetAllocator::new(1000);
        assert_eq!(allocator.total_budget, 1000);
    }

    #[test]
    fn test_greedy_allocation() {
        let allocator = BudgetAllocator::new(100).with_strategy(AllocationStrategy::Greedy);

        let content = vec![
            make_relevance("High score content with enough text", 0.9, 0),
            make_relevance("Low score content", 0.3, 0),
        ];

        let result = allocator.allocate(content, 1);
        assert!(!result.is_empty());
        assert!(result.tokens_used <= 100);
    }

    #[test]
    fn test_min_score_filter() {
        let allocator = BudgetAllocator::new(1000).with_min_score(0.5);

        let content = vec![
            make_relevance("Good content", 0.8, 0),
            make_relevance("Bad content", 0.2, 0),
        ];

        let result = allocator.allocate(content, 1);
        assert_eq!(result.selected.len(), 1);
    }

    #[test]
    fn test_truncation() {
        let allocator = BudgetAllocator::new(50);
        let truncated = allocator.truncate_content(
            "This is a very long piece of content. It has multiple sentences. We want to test truncation at sentence boundary.",
            25,  // Need at least 20 tokens for truncation
        );

        assert!(truncated.is_some());
        let text = truncated.unwrap();
        // Should truncate and add ellipsis
        assert!(text.len() < 200); // Should be truncated
    }

    #[test]
    fn test_hierarchical_allocation() {
        let allocator = BudgetAllocator::new(200)
            .with_strategy(AllocationStrategy::Hierarchical { min_per_level: 0.2 });

        let content = vec![
            make_relevance("Depth 0 content", 0.9, 0),
            make_relevance("Depth 1 content A", 0.7, 1),
            make_relevance("Depth 1 content B", 0.6, 1),
            make_relevance("Depth 2 content", 0.8, 2),
        ];

        let result = allocator.allocate(content, 2);

        // Should have content from multiple depths
        let depths: std::collections::HashSet<usize> =
            result.selected.iter().map(|s| s.depth).collect();
        assert!(depths.len() >= 2);
    }
}
