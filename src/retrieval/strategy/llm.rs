// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM-based retrieval strategy.
//!
//! Uses an LLM for deep reasoning about node relevance with ToC context.

use async_trait::async_trait;
use serde::Deserialize;

use crate::domain::{NodeId, VectorlessTree, TocView};
use crate::llm::LlmClient;
use super::super::types::{NavigationDecision, QueryComplexity};
use super::super::RetrievalContext;
use super::r#trait::{NodeEvaluation, RetrievalStrategy, StrategyCapabilities};

/// LLM response for navigation decision.
#[derive(Debug, Clone, Deserialize)]
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
/// to the query. Includes ToC context for better navigation decisions.
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::core::retriever::strategy::LlmStrategy;
/// use vectorless::llm::LlmClient;
///
/// let client = LlmClient::with_defaults();
/// let strategy = LlmStrategy::new(client)
///     .with_toc_context(true);
/// ```
pub struct LlmStrategy {
    /// The LLM client.
    client: LlmClient,
    /// System prompt for navigation.
    system_prompt: String,
    /// ToC view generator.
    toc_view: TocView,
    /// Whether to include ToC context in prompts.
    include_toc: bool,
}

impl LlmStrategy {
    /// Create a new LLM strategy.
    pub fn new(client: LlmClient) -> Self {
        Self {
            client,
            system_prompt: Self::default_system_prompt(),
            toc_view: TocView::new(),
            include_toc: true,
        }
    }

    /// Create with default LLM client.
    pub fn with_defaults() -> Self {
        Self::new(LlmClient::with_defaults())
    }

    /// Set custom system prompt.
    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        self.system_prompt = prompt;
        self
    }

    /// Enable or disable ToC context in prompts.
    pub fn with_toc_context(mut self, include: bool) -> Self {
        self.include_toc = include;
        self
    }

    /// Default system prompt for navigation.
    fn default_system_prompt() -> String {
        r#"You are a document navigation assistant. Your task is to help find the most relevant sections in a document tree.

Given a query and document context (Table of Contents + current node), determine:
1. The relevance of this node (0-100)
2. The best action: "answer" (this node contains the answer), "explore" (check children), or "skip" (not relevant)

Respond in JSON format:
{"relevance": <0-100>, "action": "<answer|explore|skip>", "reasoning": "<brief explanation>"}

Be concise and focused on finding the most relevant information."#.to_string()
    }

    /// Build the navigation prompt for a single node.
    fn build_prompt(&self, tree: &VectorlessTree, node_id: NodeId, context: &RetrievalContext) -> String {
        let node = tree.get(node_id);
        let children = tree.children(node_id);

        // Build current node info
        let node_info = if let Some(n) = node {
            let summary = if n.summary.is_empty() {
                // Use first 200 chars of content if no summary
                &n.content[..200.min(n.content.len())]
            } else {
                &n.summary
            };
            format!(
                "Title: {}\nSummary: {}\nDepth: {}\nChildren: {}",
                n.title,
                summary,
                n.depth,
                children.len()
            )
        } else {
            "Node not found".to_string()
        };

        // Build ToC context if enabled
        let toc_context = if self.include_toc {
            let toc = self.toc_view.generate_from(tree, node_id);
            let toc_markdown = self.toc_view.format_markdown(&toc);
            // Limit ToC size for token efficiency
            let toc_preview: String = toc_markdown.chars().take(1000).collect();
            format!("\n\nDocument ToC (from this node):\n```\n{}\n```\n", toc_preview)
        } else {
            String::new()
        };

        format!(
            "Query: {}\n{}Current Node:\n{}\n\nWhat is the relevance and action?",
            context.query,
            toc_context,
            node_info
        )
    }

    /// Parse LLM response to evaluation.
    fn parse_response(&self, response: &str, tree: &VectorlessTree, node_id: NodeId) -> NodeEvaluation {
        // Try to parse as JSON
        if let Ok(parsed) = serde_json::from_str::<NavigationResponse>(response) {
            let score = (parsed.relevance as f32 / 100.0).clamp(0.0, 1.0);
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

            return NodeEvaluation {
                score,
                decision,
                reasoning: parsed.reasoning,
            };
        }

        // Fallback: try to extract relevance from text
        let score = response
            .lines()
            .find_map(|line| {
                let lower = line.to_lowercase();
                if lower.contains("relevance") || lower.contains("score") {
                    lower
                        .split(|c: char| !c.is_numeric() && c != '.')
                        .filter_map(|s| s.parse::<f32>().ok())
                        .filter(|&s| (0.0..=100.0).contains(&s))
                        .map(|v| v / 100.0)
                        .next()
                } else {
                    None
                }
            })
            .unwrap_or(0.5);

        NodeEvaluation {
            score,
            decision: if tree.is_leaf(node_id) {
                NavigationDecision::ThisIsTheAnswer
            } else {
                NavigationDecision::ExploreMore
            },
            reasoning: Some(format!("Parsed from response: {}...", &response[..100.min(response.len())])),
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

        match self.client.complete(&self.system_prompt, &prompt).await {
            Ok(response) => self.parse_response(&response, tree, node_id),
            Err(e) => {
                tracing::warn!("LLM evaluation failed: {}", e);
                NodeEvaluation {
                    score: 0.5,
                    decision: if tree.is_leaf(node_id) {
                        NavigationDecision::ThisIsTheAnswer
                    } else {
                        NavigationDecision::ExploreMore
                    },
                    reasoning: Some(format!("LLM error: {}", e)),
                }
            }
        }
    }

    async fn evaluate_nodes(
        &self,
        tree: &VectorlessTree,
        node_ids: &[NodeId],
        context: &RetrievalContext,
    ) -> Vec<NodeEvaluation> {
        // Evaluate each node individually
        // TODO: Could be optimized with batch prompts
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
