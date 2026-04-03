// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Semantic (embedding-based) retrieval strategy.
//!
//! Uses vector embeddings for semantic similarity matching.

use async_trait::async_trait;

use crate::config::StrategyConfig;
use crate::domain::{NodeId, VectorlessTree};
use super::super::types::{NavigationDecision, QueryComplexity};
use super::super::RetrievalContext;
use super::r#trait::{NodeEvaluation, RetrievalStrategy, StrategyCapabilities};

/// Embedding model trait for semantic strategies.
#[async_trait]
pub trait EmbeddingModel: Send + Sync {
    /// Generate embedding for a text.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    /// Generate embeddings for multiple texts (batch).
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    /// Get the dimension of embeddings.
    fn dimension(&self) -> usize;
}

/// Embedding generation error.
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Failed to generate embedding: {0}")]
    GenerationFailed(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// Semantic retrieval strategy using embeddings.
///
/// Compares query embeddings with node content/summary embeddings
/// to find semantically similar content.
pub struct SemanticStrategy {
    /// The embedding model to use.
    model: Box<dyn EmbeddingModel>,
    /// Whether to cache embeddings.
    cache_embeddings: bool,
    /// Similarity threshold for considering a node relevant.
    similarity_threshold: f32,
    /// High similarity threshold for "answer" decision.
    high_similarity_threshold: f32,
    /// Low similarity threshold for "explore" decision.
    low_similarity_threshold: f32,
}

impl SemanticStrategy {
    /// Create a new semantic strategy with the given embedding model.
    pub fn new(model: Box<dyn EmbeddingModel>) -> Self {
        Self::with_config(model, &StrategyConfig::default())
    }

    /// Create with configuration.
    pub fn with_config(model: Box<dyn EmbeddingModel>, config: &StrategyConfig) -> Self {
        Self {
            model,
            cache_embeddings: true,
            similarity_threshold: config.similarity_threshold,
            high_similarity_threshold: config.high_similarity_threshold,
            low_similarity_threshold: config.low_similarity_threshold,
        }
    }

    /// Set whether to cache embeddings.
    pub fn with_cache(mut self, cache: bool) -> Self {
        self.cache_embeddings = cache;
        self
    }

    /// Set the similarity threshold.
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold;
        self
    }

    /// Calculate cosine similarity between two vectors.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if mag_a == 0.0 || mag_b == 0.0 {
            0.0
        } else {
            dot / (mag_a * mag_b)
        }
    }

    /// Get text to embed for a node.
    fn get_embedding_text(tree: &VectorlessTree, node_id: NodeId) -> String {
        if let Some(node) = tree.get(node_id) {
            // Prefer summary if available, otherwise use content
            if !node.summary.is_empty() {
                format!("{}: {}", node.title, node.summary)
            } else if !node.content.is_empty() {
                // Truncate long content
                let content = if node.content.len() > 500 {
                    &node.content[..500]
                } else {
                    &node.content
                };
                format!("{}: {}", node.title, content)
            } else {
                node.title.clone()
            }
        } else {
            String::new()
        }
    }
}

#[async_trait]
impl RetrievalStrategy for SemanticStrategy {
    async fn evaluate_node(
        &self,
        tree: &VectorlessTree,
        node_id: NodeId,
        context: &RetrievalContext,
    ) -> NodeEvaluation {
        let node_text = Self::get_embedding_text(tree, node_id);

        if node_text.is_empty() {
            return NodeEvaluation {
                score: 0.0,
                decision: NavigationDecision::Skip,
                reasoning: Some("Empty node".to_string()),
            };
        }

        // Get embeddings
        let query_embedding = match self.model.embed(&context.query).await {
            Ok(e) => e,
            Err(e) => {
                return NodeEvaluation {
                    score: 0.0,
                    decision: NavigationDecision::Skip,
                    reasoning: Some(format!("Embedding error: {}", e)),
                };
            }
        };

        let node_embedding = match self.model.embed(&node_text).await {
            Ok(e) => e,
            Err(e) => {
                return NodeEvaluation {
                    score: 0.0,
                    decision: NavigationDecision::Skip,
                    reasoning: Some(format!("Embedding error: {}", e)),
                };
            }
        };

        // Calculate similarity
        let similarity = Self::cosine_similarity(&query_embedding, &node_embedding);

        // Determine decision based on similarity
        let decision = if similarity > self.high_similarity_threshold {
            NavigationDecision::ThisIsTheAnswer
        } else if similarity > self.similarity_threshold {
            if tree.is_leaf(node_id) {
                NavigationDecision::ThisIsTheAnswer
            } else {
                NavigationDecision::ExploreMore
            }
        } else if similarity > self.low_similarity_threshold {
            NavigationDecision::ExploreMore
        } else {
            NavigationDecision::Skip
        };

        NodeEvaluation {
            score: similarity,
            decision,
            reasoning: Some(format!("Semantic similarity: {:.3}", similarity)),
        }
    }

    async fn evaluate_nodes(
        &self,
        tree: &VectorlessTree,
        node_ids: &[NodeId],
        context: &RetrievalContext,
    ) -> Vec<NodeEvaluation> {
        // Get query embedding once
        let query_embedding = match self.model.embed(&context.query).await {
            Ok(e) => e,
            Err(e) => {
                return node_ids
                    .iter()
                    .map(|_| NodeEvaluation {
                        score: 0.0,
                        decision: NavigationDecision::Skip,
                        reasoning: Some(format!("Embedding error: {}", e)),
                    })
                    .collect();
            }
        };

        // Collect all node texts
        let texts: Vec<String> = node_ids
            .iter()
            .map(|&id| Self::get_embedding_text(tree, id))
            .collect();

        // Batch embed all nodes
        let node_embeddings = match self.model.embed_batch(&texts).await {
            Ok(e) => e,
            Err(e) => {
                return node_ids
                    .iter()
                    .map(|_| NodeEvaluation {
                        score: 0.0,
                        decision: NavigationDecision::Skip,
                        reasoning: Some(format!("Embedding error: {}", e)),
                    })
                    .collect();
            }
        };

        // Calculate similarities and determine decisions
        node_ids
            .iter()
            .zip(node_embeddings.iter())
            .map(|(&node_id, node_embedding)| {
                let similarity = Self::cosine_similarity(&query_embedding, node_embedding);

                let decision = if similarity > 0.8 {
                    NavigationDecision::ThisIsTheAnswer
                } else if similarity > self.similarity_threshold {
                    if tree.is_leaf(node_id) {
                        NavigationDecision::ThisIsTheAnswer
                    } else {
                        NavigationDecision::ExploreMore
                    }
                } else if similarity > 0.3 {
                    NavigationDecision::ExploreMore
                } else {
                    NavigationDecision::Skip
                };

                NodeEvaluation {
                    score: similarity,
                    decision,
                    reasoning: Some(format!("Semantic similarity: {:.3}", similarity)),
                }
            })
            .collect()
    }

    fn name(&self) -> &str {
        "semantic"
    }

    fn capabilities(&self) -> StrategyCapabilities {
        StrategyCapabilities {
            uses_llm: false,
            uses_embeddings: true,
            supports_sufficiency: true,
            typical_latency_ms: 50,
        }
    }

    fn suitable_for_complexity(&self, complexity: QueryComplexity) -> bool {
        matches!(complexity, QueryComplexity::Simple | QueryComplexity::Medium)
    }
}
