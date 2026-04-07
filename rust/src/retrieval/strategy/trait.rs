// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Strategy trait definition.

use async_trait::async_trait;

use super::super::RetrievalContext;
use super::super::types::{NavigationDecision, QueryComplexity};
use crate::document::{DocumentTree, NodeId};

/// Result of evaluating a single node.
#[derive(Debug, Clone)]
pub struct NodeEvaluation {
    /// Relevance score (0.0 - 1.0).
    pub score: f32,
    /// Recommended action for this node.
    pub decision: NavigationDecision,
    /// Optional reasoning for the decision.
    pub reasoning: Option<String>,
}

impl Default for NodeEvaluation {
    fn default() -> Self {
        Self {
            score: 0.5,
            decision: NavigationDecision::ExploreMore,
            reasoning: None,
        }
    }
}

/// Capabilities of a retrieval strategy.
#[derive(Debug, Clone, Copy, Default)]
pub struct StrategyCapabilities {
    /// Whether this strategy uses LLM calls.
    pub uses_llm: bool,
    /// Whether this strategy uses embeddings.
    pub uses_embeddings: bool,
    /// Whether this strategy supports sufficiency checking.
    pub supports_sufficiency: bool,
    /// Typical latency in milliseconds.
    pub typical_latency_ms: u64,
}

/// Trait for retrieval strategies.
///
/// A strategy determines how to navigate the document tree
/// and score nodes for relevance to a query.
#[async_trait]
pub trait RetrievalStrategy: Send + Sync {
    /// Evaluate a single node's relevance to the query.
    ///
    /// This is the core method that determines how relevant
    /// a node is to the current query.
    async fn evaluate_node(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        context: &RetrievalContext,
    ) -> NodeEvaluation;

    /// Evaluate multiple nodes in batch.
    ///
    /// Default implementation calls evaluate_node for each node,
    /// but strategies can override for efficiency (e.g., batch LLM calls).
    async fn evaluate_nodes(
        &self,
        tree: &DocumentTree,
        node_ids: &[NodeId],
        context: &RetrievalContext,
    ) -> Vec<NodeEvaluation> {
        let mut results = Vec::with_capacity(node_ids.len());
        for node_id in node_ids {
            results.push(self.evaluate_node(tree, *node_id, context).await);
        }
        results
    }

    /// Get the name of this strategy.
    fn name(&self) -> &str;

    /// Get the capabilities of this strategy.
    fn capabilities(&self) -> StrategyCapabilities;

    /// Check if this strategy is suitable for the given query complexity.
    fn suitable_for_complexity(&self, complexity: QueryComplexity) -> bool;

    /// Estimate the cost of evaluating a set of nodes.
    fn estimate_cost(&self, node_count: usize) -> StrategyCost {
        StrategyCost {
            llm_calls: if self.capabilities().uses_llm {
                node_count
            } else {
                0
            },
            tokens: if self.capabilities().uses_llm {
                node_count * 200
            } else {
                0
            },
        }
    }
}

/// Cost estimate for a strategy operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct StrategyCost {
    /// Number of LLM calls.
    pub llm_calls: usize,
    /// Number of tokens.
    pub tokens: usize,
}
