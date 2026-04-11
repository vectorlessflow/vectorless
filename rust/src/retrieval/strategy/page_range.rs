// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Page-range retrieval strategy.
//!
//! Filters document nodes by page range before applying an inner strategy.
//! Useful when the user knows approximately where the information is located.

use async_trait::async_trait;

use super::r#trait::{NodeEvaluation, RetrievalStrategy, StrategyCapabilities};
use crate::document::{DocumentTree, NodeId};
use crate::retrieval::RetrievalContext;
use crate::retrieval::types::{NavigationDecision, QueryComplexity};

/// A page range for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRange {
    /// Start page (inclusive, 1-indexed).
    pub start: usize,
    /// End page (inclusive).
    pub end: usize,
}

impl PageRange {
    /// Create a new page range.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Create a range from a single page.
    pub fn single(page: usize) -> Self {
        Self {
            start: page,
            end: page,
        }
    }

    /// Create a range starting from a page to the end.
    pub fn from(start: usize) -> Self {
        Self {
            start,
            end: usize::MAX,
        }
    }

    /// Create a range from the beginning to a page.
    pub fn until(end: usize) -> Self {
        Self { start: 1, end }
    }

    /// Check if a page is within this range.
    pub fn contains(&self, page: usize) -> bool {
        page >= self.start && page <= self.end
    }

    /// Check if this range overlaps with another.
    pub fn overlaps(&self, other: &PageRange) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    /// Get the number of pages in this range.
    pub fn len(&self) -> usize {
        if self.end == usize::MAX {
            usize::MAX
        } else {
            self.end.saturating_sub(self.start) + 1
        }
    }

    /// Check if this is an empty range.
    pub fn is_empty(&self) -> bool {
        self.start > self.end
    }
}

impl Default for PageRange {
    fn default() -> Self {
        Self {
            start: 1,
            end: usize::MAX,
        }
    }
}

/// Configuration for page-range retrieval.
#[derive(Debug, Clone)]
pub struct PageRangeConfig {
    /// The page range to search within.
    pub range: PageRange,
    /// Whether to include nodes that span across the boundary.
    pub include_boundary_nodes: bool,
    /// Whether to expand the range slightly for context.
    pub expand_context_pages: usize,
    /// Minimum overlap ratio for a node to be included.
    pub min_overlap_ratio: f32,
}

impl Default for PageRangeConfig {
    fn default() -> Self {
        Self {
            range: PageRange::default(),
            include_boundary_nodes: true,
            expand_context_pages: 0,
            min_overlap_ratio: 0.1,
        }
    }
}

impl PageRangeConfig {
    /// Create a new configuration with a page range.
    pub fn new(range: PageRange) -> Self {
        Self {
            range,
            ..Default::default()
        }
    }

    /// Set the page range.
    #[must_use]
    pub fn with_range(mut self, start: usize, end: usize) -> Self {
        self.range = PageRange::new(start, end);
        self
    }

    /// Include nodes that span the boundary.
    #[must_use]
    pub fn with_boundary_nodes(mut self, include: bool) -> Self {
        self.include_boundary_nodes = include;
        self
    }

    /// Expand the range by N pages for context.
    #[must_use]
    pub fn with_context_expansion(mut self, pages: usize) -> Self {
        self.expand_context_pages = pages;
        self
    }
}

/// Page-range retrieval strategy.
///
/// Filters nodes by their page location before delegating to an inner strategy.
/// This is useful when:
/// - The user knows approximately where information is located
/// - Searching large PDFs where certain sections are known
/// - Implementing "search within pages X-Y" functionality
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::strategy::{PageRangeStrategy, KeywordStrategy, PageRange};
///
/// // Search only pages 10-20
/// let strategy = PageRangeStrategy::new(
///     Box::new(KeywordStrategy::new()),
///     PageRange::new(10, 20),
/// );
///
/// // Search from page 50 onwards
/// let strategy = PageRangeStrategy::new(
///     Box::new(LlmStrategy::new(client)),
///     PageRange::from(50),
/// );
/// ```
pub struct PageRangeStrategy {
    /// Inner strategy for filtered nodes.
    inner: Box<dyn RetrievalStrategy>,
    /// Configuration.
    config: PageRangeConfig,
}

impl PageRangeStrategy {
    /// Create a new page-range strategy.
    pub fn new(inner: Box<dyn RetrievalStrategy>, range: PageRange) -> Self {
        Self {
            inner,
            config: PageRangeConfig::new(range),
        }
    }

    /// Create with configuration.
    pub fn with_config(inner: Box<dyn RetrievalStrategy>, config: PageRangeConfig) -> Self {
        Self { inner, config }
    }

    /// Set whether to include boundary nodes.
    #[must_use]
    pub fn with_boundary_nodes(mut self, include: bool) -> Self {
        self.config.include_boundary_nodes = include;
        self
    }

    /// Set context expansion pages.
    #[must_use]
    pub fn with_context_expansion(mut self, pages: usize) -> Self {
        self.config.expand_context_pages = pages;
        self
    }

    /// Get the effective range after context expansion.
    fn effective_range(&self) -> PageRange {
        if self.config.expand_context_pages == 0 {
            return self.config.range;
        }

        PageRange {
            start: self
                .config
                .range
                .start
                .saturating_sub(self.config.expand_context_pages),
            end: self
                .config
                .range
                .end
                .saturating_add(self.config.expand_context_pages),
        }
    }

    /// Check if a node is within the page range.
    fn is_node_in_range(&self, tree: &DocumentTree, node_id: NodeId) -> bool {
        let effective_range = self.effective_range();

        if let Some(node) = tree.get(node_id) {
            // Check if node has page information
            let (start_page, end_page) = node
                .start_page
                .zip(node.end_page)
                .unwrap_or((1, usize::MAX));

            let node_range = PageRange::new(start_page, end_page);

            // Check for overlap
            if effective_range.overlaps(&node_range) {
                // Calculate overlap ratio
                let overlap_start = effective_range.start.max(node_range.start);
                let overlap_end = effective_range.end.min(node_range.end);

                if overlap_start <= overlap_end {
                    let overlap_pages = overlap_end - overlap_start + 1;
                    let node_pages = node_range.len();

                    let ratio = overlap_pages as f32 / node_pages as f32;
                    return ratio >= self.config.min_overlap_ratio;
                }
            }
        }

        // If no page info, include the node (conservative approach)
        true
    }

    /// Filter nodes by page range.
    fn filter_by_range(
        &self,
        tree: &DocumentTree,
        node_ids: &[NodeId],
    ) -> (Vec<(usize, NodeId)>, Vec<usize>) {
        let mut included = Vec::new();
        let mut excluded = Vec::new();

        for (idx, &node_id) in node_ids.iter().enumerate() {
            if self.is_node_in_range(tree, node_id) {
                included.push((idx, node_id));
            } else {
                excluded.push(idx);
            }
        }

        (included, excluded)
    }
}

#[async_trait]
impl RetrievalStrategy for PageRangeStrategy {
    async fn evaluate_node(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        context: &RetrievalContext,
    ) -> NodeEvaluation {
        // Check if node is in range
        if !self.is_node_in_range(tree, node_id) {
            return NodeEvaluation {
                score: 0.0,
                decision: NavigationDecision::Skip,
                reasoning: Some("Node outside page range".to_string()),
            };
        }

        // Delegate to inner strategy
        self.inner.evaluate_node(tree, node_id, context).await
    }

    async fn evaluate_nodes(
        &self,
        tree: &DocumentTree,
        node_ids: &[NodeId],
        context: &RetrievalContext,
    ) -> Vec<NodeEvaluation> {
        if node_ids.is_empty() {
            return Vec::new();
        }

        // Filter nodes by page range
        let (included, excluded) = self.filter_by_range(tree, node_ids);

        // Create result vector with default values
        let mut results = vec![NodeEvaluation::default(); node_ids.len()];

        // Mark excluded nodes as skipped
        for idx in &excluded {
            results[*idx] = NodeEvaluation {
                score: 0.0,
                decision: NavigationDecision::Skip,
                reasoning: Some(format!(
                    "Outside page range {}-{}",
                    self.config.range.start, self.config.range.end
                )),
            };
        }

        // Evaluate included nodes with inner strategy
        if !included.is_empty() {
            let included_ids: Vec<NodeId> = included.iter().map(|(_, id)| *id).collect();
            let inner_results = self
                .inner
                .evaluate_nodes(tree, &included_ids, context)
                .await;

            // Map results back to original positions
            for ((orig_idx, _), eval) in included.into_iter().zip(inner_results.into_iter()) {
                results[orig_idx] = eval;
            }
        }

        results
    }

    fn name(&self) -> &'static str {
        "page_range"
    }

    fn capabilities(&self) -> StrategyCapabilities {
        let inner_caps = self.inner.capabilities();
        StrategyCapabilities {
            uses_llm: inner_caps.uses_llm,
            uses_embeddings: inner_caps.uses_embeddings,
            supports_sufficiency: inner_caps.supports_sufficiency,
            typical_latency_ms: inner_caps.typical_latency_ms, // Same as inner
        }
    }

    fn suitable_for_complexity(&self, complexity: QueryComplexity) -> bool {
        self.inner.suitable_for_complexity(complexity)
    }

    fn estimate_cost(&self, node_count: usize) -> super::r#trait::StrategyCost {
        // Estimate that only a fraction of nodes are in range
        let estimated_in_range = (node_count as f32 * 0.3) as usize;
        self.inner.estimate_cost(estimated_in_range.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_range_creation() {
        let range = PageRange::new(10, 20);
        assert_eq!(range.start, 10);
        assert_eq!(range.end, 20);
    }

    #[test]
    fn test_page_range_contains() {
        let range = PageRange::new(10, 20);
        assert!(range.contains(10));
        assert!(range.contains(15));
        assert!(range.contains(20));
        assert!(!range.contains(9));
        assert!(!range.contains(21));
    }

    #[test]
    fn test_page_range_single() {
        let range = PageRange::single(5);
        assert!(range.contains(5));
        assert!(!range.contains(4));
        assert!(!range.contains(6));
    }

    #[test]
    fn test_page_range_from() {
        let range = PageRange::from(10);
        assert!(range.contains(10));
        assert!(range.contains(100));
        assert!(range.contains(usize::MAX));
        assert!(!range.contains(9));
    }

    #[test]
    fn test_page_range_until() {
        let range = PageRange::until(20);
        assert!(range.contains(1));
        assert!(range.contains(20));
        assert!(!range.contains(21));
    }

    #[test]
    fn test_page_range_overlaps() {
        let r1 = PageRange::new(10, 20);
        let r2 = PageRange::new(15, 25);
        let r3 = PageRange::new(21, 30);

        assert!(r1.overlaps(&r2));
        assert!(!r1.overlaps(&r3));
    }

    #[test]
    fn test_page_range_len() {
        let range = PageRange::new(10, 20);
        assert_eq!(range.len(), 11);
    }

    #[test]
    fn test_config_default() {
        let config = PageRangeConfig::default();
        assert_eq!(config.range.start, 1);
        assert!(config.include_boundary_nodes);
    }
}
