// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Keyword-based retrieval strategy.
//!
//! Fast, no-LLM strategy using TF-IDF style matching.

use async_trait::async_trait;
use std::collections::{HashMap, HashSet};

use crate::domain::{NodeId, DocumentTree};
use super::super::types::{NavigationDecision, QueryComplexity};
use super::super::RetrievalContext;
use super::r#trait::{NodeEvaluation, RetrievalStrategy, StrategyCapabilities};

/// Keyword-based retrieval strategy.
///
/// Uses simple term frequency matching for fast, lightweight retrieval.
/// Best for simple queries where exact term matches are sufficient.
pub struct KeywordStrategy {
    /// Whether to use bigram matching.
    use_bigrams: bool,
    /// Whether to match in summaries.
    match_summaries: bool,
}

impl Default for KeywordStrategy {
    fn default() -> Self {
        Self {
            use_bigrams: true,
            match_summaries: true,
        }
    }
}

impl KeywordStrategy {
    /// Create a new keyword strategy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Tokenize text into words.
    fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 1)
            .map(|s| s.to_string())
            .collect()
    }

    /// Generate bigrams from tokens.
    fn bigrams(tokens: &[String]) -> Vec<String> {
        tokens
            .windows(2)
            .map(|w| format!("{} {}", w[0], w[1]))
            .collect()
    }

    /// Calculate term frequency for a document.
    fn term_frequency(tokens: &[String]) -> HashMap<String, f32> {
        let mut tf = HashMap::new();
        let len = tokens.len().max(1);
        for token in tokens {
            *tf.entry(token.clone()).or_insert(0.0) += 1.0;
        }
        // Normalize by document length
        for count in tf.values_mut() {
            *count /= len as f32;
        }
        tf
    }

    /// Calculate relevance score using term overlap.
    fn calculate_score(&self, node_tokens: &[String], query_tokens: &[String]) -> f32 {
        if query_tokens.is_empty() || node_tokens.is_empty() {
            return 0.0;
        }

        let query_set: HashSet<&String> = query_tokens.iter().collect();
        let node_set: HashSet<&String> = node_tokens.iter().collect();

        // Jaccard similarity
        let intersection = query_set.intersection(&node_set).count();
        let union = query_set.union(&node_set).count();

        if union == 0 {
            0.0
        } else {
            intersection as f32 / union as f32
        }
    }

    /// Get all text from a node for matching.
    fn get_node_text(tree: &DocumentTree, node_id: NodeId) -> String {
        if let Some(node) = tree.get(node_id) {
            let mut text = format!("{} {}", node.title, node.content);
            if !node.summary.is_empty() {
                text.push_str(&format!(" {}", node.summary));
            }
            text
        } else {
            String::new()
        }
    }
}

#[async_trait]
impl RetrievalStrategy for KeywordStrategy {
    async fn evaluate_node(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        context: &RetrievalContext,
    ) -> NodeEvaluation {
        let node_text = Self::get_node_text(tree, node_id);
        let node_tokens = Self::tokenize(&node_text);

        // Calculate base score from unigram matching
        let unigram_score = self.calculate_score(&node_tokens, &context.query_tokens);

        // Optionally add bigram matching
        let bigram_score = if self.use_bigrams {
            let node_bigrams = Self::bigrams(&node_tokens);
            let query_bigrams = Self::bigrams(&context.query_tokens);
            self.calculate_score(&node_bigrams, &query_bigrams)
        } else {
            0.0
        };

        // Combine scores (weighted average)
        let final_score = if self.use_bigrams {
            0.6 * unigram_score + 0.4 * bigram_score
        } else {
            unigram_score
        };

        // Determine decision based on score and whether node has children
        let decision = if final_score > 0.7 {
            NavigationDecision::ThisIsTheAnswer
        } else if final_score > 0.3 {
            if tree.is_leaf(node_id) {
                NavigationDecision::ThisIsTheAnswer
            } else {
                NavigationDecision::ExploreMore
            }
        } else if final_score > 0.1 {
            NavigationDecision::ExploreMore
        } else {
            NavigationDecision::Skip
        };

        NodeEvaluation {
            score: final_score,
            decision,
            reasoning: Some(format!("Keyword match score: {:.3}", final_score)),
        }
    }

    fn name(&self) -> &str {
        "keyword"
    }

    fn capabilities(&self) -> StrategyCapabilities {
        StrategyCapabilities {
            uses_llm: false,
            uses_embeddings: false,
            supports_sufficiency: false,
            typical_latency_ms: 1,
        }
    }

    fn suitable_for_complexity(&self, complexity: QueryComplexity) -> bool {
        matches!(complexity, QueryComplexity::Simple)
    }
}
