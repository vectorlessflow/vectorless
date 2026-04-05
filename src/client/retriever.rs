// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document retrieval client.
//!
//! This module provides query and retrieval operations for document content.
//!
//! # Example
//!
//! ```rust,ignore
//! let retriever = RetrieverClient::new(pipeline_retriever);
//!
//! let result = retriever
//!     .query(&tree, "What is this?", RetrieveOptions::default())
//!     .await?;
//!
//! println!("Found {} results", result.results.len());
//! ```

use std::sync::Arc;

use tracing::info;

use crate::config::Config;
use crate::document::{DocumentTree, NodeId};
use crate::error::{Error, Result};
use crate::retrieval::content::ContentAggregatorConfig;
use crate::retrieval::{
    QueryComplexity, RetrievalResult, RetrieveOptions, RetrieveResponse, Retriever,
    SufficiencyLevel,
};

use super::context::ClientContext;
use super::events::{EventEmitter, QueryEvent};
use super::types::QueryResult;

/// Document retrieval client.
///
/// Provides operations for querying document content.
pub struct RetrieverClient {
    /// Pipeline retriever.
    retriever: Arc<crate::retrieval::PipelineRetriever>,

    /// Configuration reference.
    config: Arc<Config>,

    /// Event emitter.
    events: EventEmitter,

    /// Default retrieval options.
    default_options: RetrieveOptions,
}

/// Retriever configuration.
#[derive(Debug, Clone)]
pub struct RetrieverClientConfig {
    /// Default top_k for retrieval.
    pub default_top_k: usize,

    /// Default token budget.
    pub default_token_budget: usize,

    /// Content aggregator config.
    pub content_config: Option<ContentAggregatorConfig>,

    /// Enable result caching.
    pub enable_cache: bool,
}

impl Default for RetrieverClientConfig {
    fn default() -> Self {
        Self {
            default_top_k: 5,
            default_token_budget: 4000,
            content_config: None,
            enable_cache: true,
        }
    }
}

impl RetrieverClient {
    /// Create a new retriever client.
    pub fn new(retriever: crate::retrieval::PipelineRetriever, config: Arc<Config>) -> Self {
        Self {
            retriever: Arc::new(retriever),
            config,
            events: EventEmitter::new(),
            default_options: RetrieveOptions::default(),
        }
    }

    /// Create with event emitter.
    pub fn with_events(mut self, events: EventEmitter) -> Self {
        self.events = events;
        self
    }

    /// Create with configuration.
    pub fn with_config(mut self, config: RetrieverClientConfig) -> Self {
        self.default_options = RetrieveOptions::new()
            .with_top_k(config.default_top_k)
            .with_max_tokens(config.default_token_budget)
            .with_enable_cache(config.enable_cache);
        self
    }

    /// Create from existing retriever Arc.
    pub(crate) fn from_arc(
        retriever: Arc<crate::retrieval::PipelineRetriever>,
        config: Arc<Config>,
        events: EventEmitter,
    ) -> Self {
        Self {
            retriever,
            config,
            events,
            default_options: RetrieveOptions::default(),
        }
    }

    /// Query a document tree.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The retrieval pipeline fails
    pub async fn query(
        &self,
        tree: &DocumentTree,
        question: &str,
        options: &RetrieveOptions,
    ) -> Result<QueryResult> {
        self.query_with_context(tree, question, options, &ClientContext::new())
            .await
    }

    /// Query with request context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The retrieval pipeline fails
    /// - The request has timed out
    pub async fn query_with_context(
        &self,
        tree: &DocumentTree,
        question: &str,
        options: &RetrieveOptions,
        ctx: &ClientContext,
    ) -> Result<QueryResult> {
        // Check timeout
        if ctx.is_timed_out() {
            return Err(Error::Other("Request timed out".to_string()));
        }

        self.events.emit_query(QueryEvent::Started {
            query: question.to_string(),
        });

        info!("Querying: {:?}", question);

        // Apply context overrides
        let mut options = options.clone();
        if let Some(top_k) = ctx.config.top_k {
            options.top_k = top_k;
        }
        if let Some(token_budget) = ctx.config.token_budget {
            options.max_tokens = token_budget;
        }

        // Execute retrieval
        let response = self
            .retriever
            .retrieve(tree, question, &options)
            .await
            .map_err(|e| Error::Retrieval(e.to_string()))?;

        // Build result
        let result = self.build_query_result(&response);

        self.events.emit_query(QueryEvent::Complete {
            total_results: result.node_ids.len(),
            confidence: result.score,
        });

        Ok(result)
    }

    /// Build QueryResult from RetrieveResponse.
    fn build_query_result(&self, response: &RetrieveResponse) -> QueryResult {
        // Extract node IDs
        let node_ids: Vec<String> = response
            .results
            .iter()
            .filter_map(|r| r.node_id.clone())
            .collect();

        // Build content
        let content_parts: Vec<String> = response
            .results
            .iter()
            .map(|r| {
                let mut parts = vec![format!("## {}", r.title)];
                if let Some(ref content) = r.content {
                    parts.push(content.clone());
                }
                parts.join("\n\n")
            })
            .collect();

        let content = if content_parts.is_empty() {
            response.content.clone()
        } else {
            content_parts.join("\n\n---\n\n")
        };

        QueryResult {
            doc_id: String::new(), // Will be set by caller
            node_ids,
            content,
            score: response.confidence,
        }
    }

    /// Get similar nodes to a given node.
    ///
    /// Uses tree structure and content to find similar nodes.
    pub fn find_similar(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        top_k: usize,
    ) -> Result<Vec<RetrievalResult>> {
        let mut results = Vec::new();

        // Get the target node's content for comparison
        let target_content = tree
            .get(node_id)
            .map(|n| n.content.clone())
            .unwrap_or_default();

        if target_content.is_empty() {
            return Ok(results);
        }

        // Extract keywords from target content
        let target_keywords = self.extract_keywords(&target_content);

        // Search all nodes for similarity
        let root = tree.root();
        let mut stack = vec![root];

        while let Some(current_id) = stack.pop() {
            if current_id == node_id {
                // Skip the target node itself
                stack.extend(tree.children(current_id));
                continue;
            }

            if let Some(node) = tree.get(current_id) {
                let node_keywords = self.extract_keywords(&node.content);
                let similarity = self.calculate_similarity(&target_keywords, &node_keywords);

                if similarity > 0.3 {
                    results.push(
                        RetrievalResult::new(&node.title)
                            .with_node_id(format!("{:?}", current_id))
                            .with_content(node.content.clone())
                            .with_score(similarity)
                            .with_depth(tree.depth(current_id)),
                    );
                }
            }

            stack.extend(tree.children(current_id));
        }

        // Sort by score and take top_k
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(top_k);

        Ok(results)
    }

    /// Extract keywords from content.
    fn extract_keywords(&self, content: &str) -> Vec<String> {
        content
            .to_lowercase()
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .take(20)
            .map(|s| s.to_string())
            .collect()
    }

    /// Calculate similarity between keyword sets.
    fn calculate_similarity(&self, set1: &[String], set2: &[String]) -> f32 {
        if set1.is_empty() || set2.is_empty() {
            return 0.0;
        }

        let set1_set: std::collections::HashSet<_> = set1.iter().collect();
        let set2_set: std::collections::HashSet<_> = set2.iter().collect();

        let intersection = set1_set.intersection(&set2_set).count();
        let union = set1_set.union(&set2_set).count();

        intersection as f32 / union as f32
    }

    /// Get node context (ancestors and siblings).
    ///
    /// Returns the node's ancestors up to the specified depth,
    /// along with sibling nodes at each level.
    pub fn get_node_context(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        ancestor_depth: usize,
    ) -> Result<NodeContext> {
        let mut ancestors = Vec::new();
        let mut siblings = Vec::new();

        // Get ancestors
        let mut current_id = Some(node_id);
        let mut depth = 0;

        while let Some(id) = current_id {
            if depth >= ancestor_depth {
                break;
            }

            if let Some(node) = tree.get(id) {
                ancestors.push(
                    RetrievalResult::new(&node.title)
                        .with_node_id(format!("{:?}", id))
                        .with_depth(tree.depth(id)),
                );

                // Get siblings at this level
                if let Some(parent_id) = tree.parent(id) {
                    for child_id in tree.children(parent_id) {
                        if child_id != id {
                            if let Some(sibling) = tree.get(child_id) {
                                siblings.push(
                                    RetrievalResult::new(&sibling.title)
                                        .with_node_id(format!("{:?}", child_id))
                                        .with_depth(tree.depth(child_id)),
                                );
                            }
                        }
                    }
                }
            }

            current_id = tree.parent(id);
            depth += 1;
        }

        // Get the target node
        let target = tree.get(node_id).map(|n| {
            RetrievalResult::new(&n.title)
                .with_node_id(format!("{:?}", node_id))
                .with_content(n.content.clone())
                .with_depth(tree.depth(node_id))
        });

        Ok(NodeContext {
            target,
            ancestors,
            siblings,
        })
    }

    /// Get the underlying retriever Arc.
    pub(crate) fn inner(&self) -> Arc<crate::retrieval::PipelineRetriever> {
        Arc::clone(&self.retriever)
    }
}

impl Clone for RetrieverClient {
    fn clone(&self) -> Self {
        Self {
            retriever: Arc::clone(&self.retriever),
            config: Arc::clone(&self.config),
            events: self.events.clone(),
            default_options: self.default_options.clone(),
        }
    }
}

/// Node context information.
#[derive(Debug, Clone)]
pub struct NodeContext {
    /// The target node.
    pub target: Option<RetrievalResult>,

    /// Ancestor nodes (ordered from parent to root).
    pub ancestors: Vec<RetrievalResult>,

    /// Sibling nodes at each ancestor level.
    pub siblings: Vec<RetrievalResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retriever_client_creation() {
        let config = Arc::new(Config::default());
        let retriever = crate::retrieval::PipelineRetriever::new();
        let client = RetrieverClient::new(retriever, config);
        assert!(client.default_options.top_k > 0);
    }
}
