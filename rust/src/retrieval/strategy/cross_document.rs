// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Cross-document retrieval strategy.
//!
//! Retrieves relevant content from multiple documents, aggregating
//! results into a unified response.

use async_trait::async_trait;
use std::sync::Arc;

use super::r#trait::{NodeEvaluation, RetrievalStrategy, StrategyCapabilities};
use crate::document::{DocumentTree, NodeId};
use crate::graph::DocumentGraph;
use crate::retrieval::RetrievalContext;
use crate::retrieval::types::QueryComplexity;

/// Document identifier for cross-document retrieval.
pub type DocumentId = String;

/// A document with its tree structure for cross-document retrieval.
pub struct DocumentEntry {
    /// Unique document identifier.
    pub id: DocumentId,
    /// Document title or name.
    pub title: String,
    /// The document tree.
    pub tree: DocumentTree,
}

impl DocumentEntry {
    /// Create a new document entry.
    pub fn new(id: impl Into<String>, title: impl Into<String>, tree: DocumentTree) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            tree,
        }
    }
}

/// Result from a single document in cross-document retrieval.
#[derive(Debug, Clone)]
pub struct DocumentResult {
    /// Document ID.
    pub doc_id: DocumentId,
    /// Document title.
    pub doc_title: String,
    /// Node evaluation results from this document.
    pub evaluations: Vec<(NodeId, NodeEvaluation)>,
    /// Best score from this document.
    pub best_score: f32,
}

/// Strategy for merging results from multiple documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MergeStrategy {
    /// Take top-k results across all documents (default).
    #[default]
    TopK,
    /// Take best result from each document.
    BestPerDocument,
    /// Weight results by document relevance score.
    WeightedByRelevance,
    /// Use graph connectivity to boost connected documents.
    GraphBoosted,
}

/// Configuration for cross-document retrieval.
#[derive(Debug, Clone)]
pub struct CrossDocumentConfig {
    /// Maximum number of documents to search.
    pub max_documents: usize,
    /// Maximum results per document.
    pub max_results_per_doc: usize,
    /// Maximum total results.
    pub max_total_results: usize,
    /// Minimum score threshold for including results.
    pub min_score: f32,
    /// How to merge results from multiple documents.
    pub merge_strategy: MergeStrategy,
    /// Whether to search documents in parallel.
    pub parallel_search: bool,
}

impl Default for CrossDocumentConfig {
    fn default() -> Self {
        Self {
            max_documents: 10,
            max_results_per_doc: 3,
            max_total_results: 10,
            min_score: 0.3,
            merge_strategy: MergeStrategy::TopK,
            parallel_search: true,
        }
    }
}

/// Cross-document retrieval strategy.
///
/// Searches multiple documents and aggregates results based on
/// the configured merge strategy.
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::strategy::{CrossDocumentStrategy, DocumentEntry};
///
/// let docs = vec![
///     DocumentEntry::new("doc1", "Manual A", tree1),
///     DocumentEntry::new("doc2", "Manual B", tree2),
/// ];
///
/// let strategy = CrossDocumentStrategy::new(inner_strategy)
///     .with_config(CrossDocumentConfig {
///         max_documents: 5,
///         max_results_per_doc: 2,
///         ..Default::default()
///     });
/// ```
pub struct CrossDocumentStrategy {
    /// Inner strategy for searching individual documents.
    inner: Box<dyn RetrievalStrategy>,
    /// Configuration.
    config: CrossDocumentConfig,
    /// Documents to search.
    documents: Vec<DocumentEntry>,
    /// Optional document graph for graph-aware ranking.
    graph: Option<Arc<DocumentGraph>>,
}

impl CrossDocumentStrategy {
    /// Create a new cross-document strategy.
    pub fn new(inner: Box<dyn RetrievalStrategy>) -> Self {
        Self {
            inner,
            config: CrossDocumentConfig::default(),
            documents: Vec::new(),
            graph: None,
        }
    }

    /// Create with configuration.
    pub fn with_config(mut self, config: CrossDocumentConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a document to search.
    pub fn add_document(&mut self, doc: DocumentEntry) {
        if self.documents.len() < self.config.max_documents {
            self.documents.push(doc);
        }
    }

    /// Set documents to search.
    pub fn with_documents(mut self, documents: Vec<DocumentEntry>) -> Self {
        self.documents = documents
            .into_iter()
            .take(self.config.max_documents)
            .collect();
        self
    }

    /// Get the number of documents.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Set the document graph for graph-aware ranking.
    pub fn with_graph(mut self, graph: Arc<DocumentGraph>) -> Self {
        self.graph = Some(graph);
        self
    }

    /// Apply graph-based score boosting to merged results.
    ///
    /// For each high-confidence result (score > 0.5), find its graph neighbors
    /// and boost their scores by `boost_factor * edge_weight`.
    fn apply_graph_boost(
        &self,
        results: &mut Vec<(DocumentId, NodeId, NodeEvaluation)>,
        boost_factor: f32,
    ) {
        let graph = match self.graph {
            Some(ref g) => g,
            None => return,
        };

        // Collect doc_ids with high scores
        let high_score_docs: Vec<(String, f32)> = results
            .iter()
            .filter(|(_, _, eval)| eval.score > 0.5)
            .map(|(doc_id, _, eval)| (doc_id.clone(), eval.score))
            .collect();

        if high_score_docs.is_empty() {
            return;
        }

        // For each high-score doc, boost its graph neighbors
        for (doc_id, base_score) in &high_score_docs {
            let neighbors = graph.get_neighbors(doc_id);
            for edge in neighbors {
                // Find results from the neighbor doc and boost them
                for result in results.iter_mut() {
                    if result.0 == edge.target_doc_id {
                        let boost = boost_factor * edge.weight * base_score;
                        result.2.score += boost;
                    }
                }
            }
        }

        // Re-sort by score after boosting
        results.sort_by(|a, b| {
            b.2.score
                .partial_cmp(&a.2.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Search a single document and return results.
    ///
    /// Performs depth-first traversal: evaluates top-level nodes first,
    /// then recursively explores children of high-scoring nodes.
    async fn search_document(
        &self,
        doc: &DocumentEntry,
        context: &RetrievalContext,
    ) -> DocumentResult {
        let root_id = doc.tree.root();
        let children = doc.tree.children(root_id);

        // Phase 1: Evaluate top-level nodes
        let top_evaluations = self
            .inner
            .evaluate_nodes(&doc.tree, &children, context)
            .await;

        let mut scored_nodes: Vec<(NodeId, NodeEvaluation)> = children
            .into_iter()
            .zip(top_evaluations.into_iter())
            .filter(|(_, eval)| eval.score >= self.config.min_score)
            .collect();

        // Phase 2: Depth traversal — explore children of high-scoring nodes
        let high_score_nodes: Vec<NodeId> = scored_nodes
            .iter()
            .filter(|(_, eval)| eval.score >= self.config.min_score * 1.5)
            .map(|(id, _)| *id)
            .collect();

        for node_id in high_score_nodes {
            let depth_results = self
                .search_subtree(&doc.tree, node_id, context, 0, 2)
                .await;
            scored_nodes.extend(depth_results);
        }

        // Sort by score descending
        scored_nodes.sort_by(|a, b| {
            b.1.score
                .partial_cmp(&a.1.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Deduplicate by node_id
        scored_nodes.dedup_by(|a, b| a.0 == b.0);

        // Limit results per document
        scored_nodes.truncate(self.config.max_results_per_doc);

        let best_score = scored_nodes.first().map(|(_, e)| e.score).unwrap_or(0.0);

        DocumentResult {
            doc_id: doc.id.clone(),
            doc_title: doc.title.clone(),
            evaluations: scored_nodes,
            best_score,
        }
    }

    /// Recursively search a subtree, evaluating children of high-scoring nodes.
    fn search_subtree<'a>(
        &'a self,
        tree: &'a DocumentTree,
        parent_id: NodeId,
        context: &'a RetrievalContext,
        current_depth: usize,
        max_depth: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<(NodeId, NodeEvaluation)>> + Send + 'a>> {
        Box::pin(async move {
        if current_depth >= max_depth {
            return Vec::new();
        }

        let children = tree.children(parent_id);
        if children.is_empty() {
            return Vec::new();
        }

        let evaluations = self
            .inner
            .evaluate_nodes(tree, &children, context)
            .await;

        let mut results = Vec::new();
        let mut explore_further = Vec::new();

        for (node_id, eval) in children.into_iter().zip(evaluations.into_iter()) {
            if eval.score >= self.config.min_score {
                results.push((node_id, eval.clone()));
            }
            // Only explore deeper if score is promising
            if eval.score >= self.config.min_score * 1.5 {
                explore_further.push(node_id);
            }
        }

        // Recurse into promising children
        for child_id in explore_further {
            let deeper = self
                .search_subtree(tree, child_id, context, current_depth + 1, max_depth)
                .await;
            results.extend(deeper);
        }

        results
        })
    }

    /// Merge results from all documents.
    fn merge_results(
        &self,
        doc_results: Vec<DocumentResult>,
    ) -> Vec<(DocumentId, NodeId, NodeEvaluation)> {
        match self.config.merge_strategy {
            MergeStrategy::TopK => {
                // Collect all results and sort by score
                let mut all_results: Vec<_> = doc_results
                    .into_iter()
                    .flat_map(|doc| {
                        doc.evaluations
                            .into_iter()
                            .map(move |(node_id, eval)| (doc.doc_id.clone(), node_id, eval))
                    })
                    .collect();

                all_results.sort_by(|a, b| {
                    b.2.score
                        .partial_cmp(&a.2.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                all_results.truncate(self.config.max_total_results);
                all_results
            }

            MergeStrategy::BestPerDocument => {
                // Take the best result from each document
                doc_results
                    .into_iter()
                    .filter_map(|doc| {
                        doc.evaluations
                            .into_iter()
                            .next()
                            .map(|(node_id, eval)| (doc.doc_id, node_id, eval))
                    })
                    .take(self.config.max_total_results)
                    .collect()
            }

            MergeStrategy::WeightedByRelevance => {
                // Weight by document's best score
                let max_doc_score = doc_results
                    .iter()
                    .map(|d| d.best_score)
                    .fold(0.0_f32, f32::max);

                let mut all_results: Vec<_> = doc_results
                    .into_iter()
                    .flat_map(|doc| {
                        let weight = if max_doc_score > 0.0 {
                            doc.best_score / max_doc_score
                        } else {
                            1.0
                        };
                        doc.evaluations.into_iter().map(move |(node_id, mut eval)| {
                            eval.score *= weight;
                            (doc.doc_id.clone(), node_id, eval)
                        })
                    })
                    .collect();

                all_results.sort_by(|a, b| {
                    b.2.score
                        .partial_cmp(&a.2.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                all_results.truncate(self.config.max_total_results);
                all_results
            }

            MergeStrategy::GraphBoosted => {
                // First do TopK merge
                let mut all_results: Vec<_> = doc_results
                    .into_iter()
                    .flat_map(|doc| {
                        doc.evaluations
                            .into_iter()
                            .map(move |(node_id, eval)| (doc.doc_id.clone(), node_id, eval))
                    })
                    .collect();

                all_results.sort_by(|a, b| {
                    b.2.score
                        .partial_cmp(&a.2.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                // Apply graph-based boosting
                self.apply_graph_boost(&mut all_results, 0.15);

                all_results.truncate(self.config.max_total_results);
                all_results
            }
        }
    }
}

#[async_trait]
impl RetrievalStrategy for CrossDocumentStrategy {
    async fn evaluate_node(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        context: &RetrievalContext,
    ) -> NodeEvaluation {
        // Delegate to inner strategy
        self.inner.evaluate_node(tree, node_id, context).await
    }

    async fn evaluate_nodes(
        &self,
        tree: &DocumentTree,
        node_ids: &[NodeId],
        context: &RetrievalContext,
    ) -> Vec<NodeEvaluation> {
        // Delegate to inner strategy
        self.inner.evaluate_nodes(tree, node_ids, context).await
    }

    fn name(&self) -> &'static str {
        "cross_document"
    }

    fn capabilities(&self) -> StrategyCapabilities {
        let inner_caps = self.inner.capabilities();
        StrategyCapabilities {
            uses_llm: inner_caps.uses_llm,
            uses_embeddings: inner_caps.uses_embeddings,
            supports_sufficiency: true,
            typical_latency_ms: inner_caps.typical_latency_ms * self.documents.len().min(5) as u64,
        }
    }

    fn suitable_for_complexity(&self, complexity: QueryComplexity) -> bool {
        // Cross-document is suitable for all complexity levels
        matches!(
            complexity,
            QueryComplexity::Simple | QueryComplexity::Medium | QueryComplexity::Complex
        )
    }

    fn estimate_cost(&self, node_count: usize) -> super::r#trait::StrategyCost {
        let inner_cost = self.inner.estimate_cost(node_count);
        super::r#trait::StrategyCost {
            llm_calls: inner_cost.llm_calls * self.documents.len().min(self.config.max_documents),
            tokens: inner_cost.tokens * self.documents.len().min(self.config.max_documents),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = CrossDocumentConfig::default();
        assert_eq!(config.max_documents, 10);
        assert_eq!(config.max_results_per_doc, 3);
        assert_eq!(config.max_total_results, 10);
        assert_eq!(config.merge_strategy, MergeStrategy::TopK);
    }

    #[test]
    fn test_merge_strategy_default() {
        let strategy = MergeStrategy::default();
        assert!(matches!(strategy, MergeStrategy::TopK));
    }
}
