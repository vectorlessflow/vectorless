// Copyright (c) 2026 vectorless developers
//! LLM-based retrieval strategy.
//!
//! Uses an LLM for deep reasoning about node relevance.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::core::{NodeId, VectorlessTree};
use super::super::types::{NavigationDecision, QueryComplexity};
use super::super::RetrievalContext;
use super::{NodeEvaluation, RetrievalStrategy, StrategyCapabilities};

/// LLM client trait for the strategy.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Generate a completion for the given prompt.
    async fn complete(&self, prompt: &str) -> Result<String, LlmError>;

    /// Get the model name.
    fn model_name(&self) -> &str;
}

/// LLM error types.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("LLM request failed: {0}")]
    RequestFailed(String),
    #[error("Failed to parse LLM response: {0}")]
    ParseError(String),
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
}

/// LLM response for navigation decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NavigationResponse {
    /// Relevance score (0-100, will be normalized to 0-1).
    relevance: u8,
    /// Decision: "answer", "explore", or "skip".
    action: String,
    /// Optional reasoning.
    #[serde(default)]
    reasoning: Option<String>,
}

/// LLM-based retrieval strategy.
///
/// Uses an LLM to reason about which nodes are most relevant
/// to the query. Most accurate but also most expensive.
pub struct LlmStrategy {
    /// The LLM client.
    client: Box<dyn LlmClient>,
    /// System prompt for navigation.
    system_prompt: String,
    /// Maximum tokens for LLM responses.
    max_response_tokens: usize,
}

impl LlmStrategy {
    /// Create a new LLM strategy.
    pub fn new(client: Box<dyn LlmClient>) -> Self {
        Self {
            client,
            system_prompt: Self::default_system_prompt(),
            max_response_tokens: 150,
        }
    }

    /// Set custom system prompt.
    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        self.system_prompt = prompt;
        self
    }

    /// Default system prompt for navigation.
    fn default_system_prompt() -> String {
        r#"You are a document navigation assistant. Your task is to help find the most relevant sections in a document tree.

Given a query and a list of node titles with their summaries, determine:
1. The relevance of each node (0-100)
2. The best action: "answer" (this node contains the answer), "explore" (check children), or "skip" (not relevant)

Respond in JSON format:
{"relevance": <0-100>, "action": "<answer|explore|skip>", "reasoning": "<brief explanation>"}

Be concise and focused on finding the most relevant information."#.to_string()
    }

    /// Build the navigation prompt for a single node.
    fn build_prompt(&self, tree: &VectorlessTree, node_id: NodeId, context: &RetrievalContext) -> String {
        let node = tree.get(node_id);
        let children = tree.children(node_id);

        let node_info = if let Some(n) = node {
            format!(
                "Title: {}\nSummary: {}\nDepth: {}\nChildren count: {}",
                n.title,
                if n.summary.is_empty() { "N/A" } else { &n.summary },
                n.depth,
                children.len()
            )
        } else {
            "Node not found".to_string()
        };

        format!(
            "{}\n\nQuery: {}\n\nCurrent Node:\n{}\n\nWhat is the relevance and action?",
            self.system_prompt,
            context.query,
            node_info
        )
    }

    /// Parse LLM response to evaluation.
    fn parse_response(&self, response: &str, tree: &VectorlessTree, node_id: NodeId) -> NodeEvaluation {
        // Try to parse as JSON
        if let Ok(parsed) = serde_json::from_str::<NavigationResponse>(response) {
            let score = parsed.relevance as f32 / 100.0;
            let decision = match parsed.action.to_lowercase().as_str() {
                "answer" => NavigationDecision::ThisIsTheAnswer,
                "explore" => {
                    if tree.is_leaf(node_id) {
                        NavigationDecision::ThisIsTheAnswer
                    } else {
                        NavigationDecision::ExploreMore
                    }
                }
                _ => NavigationDecision::Skip,
            };

            NodeEvaluation {
                score,
                decision,
                reasoning: parsed.reasoning,
            }
        } else {
            // Fallback: try to extract relevance from text
            let score = response
                .lines()
                .find(|line| line.to_lowercase().contains("relevance"))
                .and_then(|line| {
                    line.split(|c: char| !c.is_numeric())
                        .filter_map(|s| s.parse::<u8>().ok())
                        .next()
                })
                .map(|v| v as f32 / 100.0)
                .unwrap_or(0.5);

            NodeEvaluation {
                score,
                decision: NavigationDecision::ExploreMore,
                reasoning: Some(format!("Failed to parse response: {}", response)),
            }
        }
    }
}

#[async_trait]
impl RetrievalStrategy for LlmStrategy {
    async fn evaluate_node(
        &self,
        tree: &VectorlessTree,
        node_id: NodeId,
        context: &RetrievalContext,
    ) -> NodeEvaluation {
        let prompt = self.build_prompt(tree, node_id, context);

        match self.client.complete(&prompt).await {
            Ok(response) => self.parse_response(&response, tree, node_id),
            Err(e) => NodeEvaluation {
                score: 0.0,
                decision: NavigationDecision::Skip,
                reasoning: Some(format!("LLM error: {}", e)),
            },
        }
    }

    async fn evaluate_nodes(
        &self,
        tree: &VectorlessTree,
        node_ids: &[NodeId],
        context: &RetrievalContext,
    ) -> Vec<NodeEvaluation> {
        // For LLM strategy, evaluate each node individually
        // Could be optimized with batch prompts in the future
        let mut results = Vec::with_capacity(node_ids.len());
        for node_id in node_ids {
            results.push(self.evaluate_node(tree, *node_id, context).await);
        }
        results
    }

    fn name(&self) -> &str {
        "llm"
    }

    fn capabilities(&self) -> StrategyCapabilities {
        StrategyCapabilities {
            uses_llm: true,
            uses_embeddings: false,
            supports_sufficiency: true,
            typical_latency_ms: 500,
        }
    }

    fn suitable_for_complexity(&self, complexity: QueryComplexity) -> bool {
        matches!(complexity, QueryComplexity::Medium | QueryComplexity::Complex)
    }
}
