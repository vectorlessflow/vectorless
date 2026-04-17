// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM-based retrieval strategy.
//!
//! Uses an LLM for deep reasoning about node relevance with ToC context.
//! Supports batch evaluation — all sibling nodes are scored in a single
//! LLM call instead of one call per node.

use async_trait::async_trait;
use serde::Deserialize;

use super::super::RetrievalContext;
use super::super::types::{NavigationDecision, QueryComplexity};
use super::r#trait::{NodeEvaluation, RetrievalStrategy, StrategyCapabilities};
use crate::document::{DocumentTree, NodeId, TocView};
use crate::llm::LlmClient;
use crate::llm::memo::{MemoKey, MemoOpType, MemoStore, MemoValue};
use crate::utils::fingerprint::Fingerprint;

/// LLM response for a single node in batch evaluation.
#[derive(Debug, Clone, Deserialize)]
struct NodeScore {
    /// 1-based index matching the order in the prompt.
    index: usize,
    /// Relevance score (0-100, will be normalized to 0-1).
    relevance: u8,
    /// Decision: "answer", "explore", or "skip".
    action: String,
    /// Optional reasoning.
    #[serde(default)]
    reasoning: Option<String>,
}

/// LLM response for batch node evaluation.
#[derive(Debug, Clone, Deserialize)]
struct BatchResponse {
    /// Analysis reasoning.
    #[serde(default)]
    reasoning: String,
    /// Scored nodes.
    nodes: Vec<NodeScore>,
}

/// LLM response for single-node evaluation (fallback).
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
/// # Batch Evaluation
///
/// When multiple nodes need scoring, they are sent in a single LLM call
/// instead of one call per node. This reduces latency from O(N) LLM calls
/// to O(1).
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::retrieval::strategy::LlmStrategy;
/// use vectorless::llm::LlmClient;
///
/// let client = LlmClient::with_defaults();
/// let strategy = LlmStrategy::new(client)
///     .with_toc_context(true);
/// ```
#[derive(Clone)]
pub struct LlmStrategy {
    /// The LLM client.
    client: LlmClient,
    /// System prompt for single-node navigation.
    system_prompt: String,
    /// System prompt for batch evaluation.
    batch_system_prompt: String,
    /// ToC view generator.
    toc_view: TocView,
    /// Whether to include ToC context in prompts.
    include_toc: bool,
    /// Memo store for caching LLM evaluations.
    memo_store: Option<MemoStore>,
}

impl LlmStrategy {
    /// Create a new LLM strategy.
    pub fn new(client: LlmClient) -> Self {
        Self {
            client,
            system_prompt: Self::default_system_prompt(),
            batch_system_prompt: Self::default_batch_system_prompt(),
            toc_view: TocView::new(),
            include_toc: true,
            memo_store: None,
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

    /// Add memo store for caching LLM evaluations.
    ///
    /// When enabled, node evaluations are cached based on prompt fingerprints,
    /// avoiding redundant LLM calls for the same node+query combinations.
    pub fn with_memo_store(mut self, store: MemoStore) -> Self {
        self.memo_store = Some(store);
        self
    }

    /// Default system prompt for single-node navigation.
    fn default_system_prompt() -> String {
        r#"You are a document navigation assistant. Your task is to help find the most relevant sections in a document tree.

Given a query and document context (Table of Contents + current node), determine:
1. The relevance of this node (0-100)
2. The best action: "answer" (this node contains the answer), "explore" (check children), or "skip" (not relevant)

Respond in JSON format:
{"relevance": <0-100>, "action": "<answer|explore|skip>", "reasoning": "<brief explanation>"}

Be concise and focused on finding the most relevant information."#.to_string()
    }

    /// Default system prompt for batch node evaluation.
    fn default_batch_system_prompt() -> String {
        r#"You are a document navigation assistant. Score the relevance of multiple document sections against a user query.

CRITICAL: Respond with ONLY valid JSON (no markdown code blocks).

Response format:
{
  "reasoning": "Brief analysis of the query",
  "nodes": [
    {"index": 1, "relevance": 85, "action": "answer", "reason": "Why relevant"},
    {"index": 2, "relevance": 30, "action": "skip", "reason": "Why not relevant"}
  ]
}

Rules:
- index: MUST be the number from [N] brackets in the input
- relevance: 0-100 (how relevant this section is to the query)
- action: one of "answer", "explore", "skip"
- Score ALL provided nodes, not just the top ones
- Be concise in reasons"#.to_string()
    }

    /// Build the navigation prompt for a single node.
    fn build_prompt(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        context: &RetrievalContext,
    ) -> String {
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
            format!(
                "\n\nDocument ToC (from this node):\n```\n{}\n```\n",
                toc_preview
            )
        } else {
            String::new()
        };

        format!(
            "Query: {}\n{}Current Node:\n{}\n\nWhat is the relevance and action?",
            context.query, toc_context, node_info
        )
    }

    /// Build a batch prompt that presents all nodes at once.
    fn build_batch_prompt(
        &self,
        tree: &DocumentTree,
        node_ids: &[NodeId],
        context: &RetrievalContext,
    ) -> String {
        // Collect node descriptions
        let node_descriptions: Vec<String> = node_ids
            .iter()
            .enumerate()
            .filter_map(|(i, &node_id)| {
                let node = tree.get(node_id)?;
                let children = tree.children(node_id);
                let summary = if node.summary.is_empty() {
                    let end = 200.min(node.content.len());
                    &node.content[..end]
                } else {
                    &node.summary
                };
                Some(format!(
                    "[{}] Title: \"{}\"\n     Summary: \"{}\"\n     Depth: {}, Children: {}",
                    i + 1,
                    node.title,
                    summary,
                    node.depth,
                    children.len()
                ))
            })
            .collect();

        let nodes_str = node_descriptions.join("\n\n");

        // Optional ToC context from the first node's parent scope
        let toc_context = if self.include_toc && !node_ids.is_empty() {
            let toc = self.toc_view.generate_from(tree, node_ids[0]);
            let toc_markdown = self.toc_view.format_markdown(&toc);
            let toc_preview: String = toc_markdown.chars().take(800).collect();
            format!("\n\nDocument ToC:\n{}\n", toc_preview)
        } else {
            String::new()
        };

        format!(
            "USER QUERY: {}\n{}SECTIONS TO SCORE ({} entries):\n{}\n\nScore ALL sections. Respond with ONLY the JSON object:",
            context.query,
            toc_context,
            node_ids.len(),
            nodes_str
        )
    }

    /// Build a memo cache key for a single node evaluation.
    fn node_eval_cache_key(&self, node_id: NodeId, context: &RetrievalContext) -> MemoKey {
        let mut parts = String::new();
        parts.push_str(&context.query);
        parts.push_str(":node:");
        // Use the NodeId debug representation as part of the fingerprint
        parts.push_str(&format!("{:?}", node_id));
        let fp = Fingerprint::from_str(&parts);
        MemoKey {
            op_type: MemoOpType::NodeEvaluation,
            input_fp: fp,
            model_id: None,
            version: 1,
            context_fp: Fingerprint::zero(),
        }
    }

    /// Build a memo cache key for a batch evaluation.
    fn batch_eval_cache_key(&self, node_ids: &[NodeId], context: &RetrievalContext) -> MemoKey {
        let mut parts = String::new();
        parts.push_str(&context.query);
        parts.push_str(":batch:");
        for id in node_ids {
            parts.push_str(&format!("{:?}", id));
            parts.push(',');
        }
        let fp = Fingerprint::from_str(&parts);
        MemoKey {
            op_type: MemoOpType::NodeEvaluation,
            input_fp: fp,
            model_id: None,
            version: 1,
            context_fp: Fingerprint::zero(),
        }
    }

    /// Try to deserialize a cached NodeEvaluation from MemoValue.
    fn deserialize_cached_eval(&self, value: &MemoValue) -> Option<NodeEvaluation> {
        match value {
            MemoValue::Json(json) => serde_json::from_value(json.clone()).ok(),
            _ => None,
        }
    }

    /// Parse LLM response to evaluation for a single node.
    fn parse_response(
        &self,
        response: &str,
        tree: &DocumentTree,
        node_id: NodeId,
    ) -> NodeEvaluation {
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
            reasoning: Some(format!(
                "Parsed from response: {}...",
                &response[..100.min(response.len())]
            )),
        }
    }

    /// Parse a batch LLM response into per-node evaluations.
    ///
    /// Returns evaluations in the same order as the input `node_ids`.
    /// Nodes that the LLM didn't score get a default evaluation.
    fn parse_batch_response(
        &self,
        response: &str,
        tree: &DocumentTree,
        node_ids: &[NodeId],
    ) -> Vec<NodeEvaluation> {
        // Try JSON parse
        if let Ok(batch) = serde_json::from_str::<BatchResponse>(response) {
            let mut evaluations = vec![
                NodeEvaluation {
                    score: 0.3,
                    decision: NavigationDecision::ExploreMore,
                    reasoning: Some("Not scored by LLM (batch fallback)".to_string()),
                };
                node_ids.len()
            ];

            for node_score in batch.nodes {
                let idx = node_score.index.saturating_sub(1);
                if idx < node_ids.len() {
                    let node_id = node_ids[idx];
                    let score = (node_score.relevance as f32 / 100.0).clamp(0.0, 1.0);
                    let decision = match node_score.action.to_lowercase().as_str() {
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
                    evaluations[idx] = NodeEvaluation {
                        score,
                        decision,
                        reasoning: node_score.reasoning,
                    };
                }
            }

            return evaluations;
        }

        // Fallback: could not parse batch, return defaults
        tracing::warn!(
            "Failed to parse batch LLM response, using defaults for {} nodes",
            node_ids.len()
        );
        node_ids
            .iter()
            .map(|&node_id| NodeEvaluation {
                score: 0.5,
                decision: if tree.is_leaf(node_id) {
                    NavigationDecision::ThisIsTheAnswer
                } else {
                    NavigationDecision::ExploreMore
                },
                reasoning: Some("Batch parse fallback".to_string()),
            })
            .collect()
    }
}

#[async_trait]
impl RetrievalStrategy for LlmStrategy {
    async fn evaluate_node(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        context: &RetrievalContext,
    ) -> NodeEvaluation {
        // Check memo cache
        if let Some(ref store) = self.memo_store {
            let cache_key = self.node_eval_cache_key(node_id, context);
            if let Some(cached) = store.get(&cache_key) {
                if let Some(eval) = self.deserialize_cached_eval(&cached) {
                    tracing::debug!("Memo cache hit for node evaluation (node={:?})", node_id);
                    return eval;
                }
            }
        }

        let prompt = self.build_prompt(tree, node_id, context);

        let result = match self.client.complete(&self.system_prompt, &prompt).await {
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
        };

        // Cache the result
        if let Some(ref store) = self.memo_store {
            let cache_key = self.node_eval_cache_key(node_id, context);
            if let Ok(json) = serde_json::to_value(&result) {
                let tokens = (prompt.len() / 4) as u64;
                store.put_with_tokens(cache_key, MemoValue::Json(json), tokens);
            }
        }

        result
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

        // Single node: use the simpler single-node prompt
        if node_ids.len() == 1 {
            return vec![self.evaluate_node(tree, node_ids[0], context).await];
        }

        // Check memo cache for the entire batch
        if let Some(ref store) = self.memo_store {
            let cache_key = self.batch_eval_cache_key(node_ids, context);
            if let Some(cached) = store.get(&cache_key) {
                if let MemoValue::Json(json) = &cached {
                    if let Ok(evals) = serde_json::from_value::<Vec<NodeEvaluation>>(json.clone()) {
                        if evals.len() == node_ids.len() {
                            tracing::debug!(
                                "Memo cache hit for batch evaluation ({} nodes)",
                                node_ids.len()
                            );
                            return evals;
                        }
                    }
                }
            }
        }

        // Batch: send all nodes in one LLM call
        let prompt = self.build_batch_prompt(tree, node_ids, context);

        let result = match self
            .client
            .complete(&self.batch_system_prompt, &prompt)
            .await
        {
            Ok(response) => self.parse_batch_response(&response, tree, node_ids),
            Err(e) => {
                tracing::warn!(
                    "Batch LLM evaluation failed ({}), falling back to single evaluation: {}",
                    node_ids.len(),
                    e
                );
                // Fallback: evaluate individually (still works, just slower)
                let mut results = Vec::with_capacity(node_ids.len());
                for &node_id in node_ids {
                    results.push(self.evaluate_node(tree, node_id, context).await);
                }
                results
            }
        };

        // Cache the batch result
        if let Some(ref store) = self.memo_store {
            let cache_key = self.batch_eval_cache_key(node_ids, context);
            if let Ok(json) = serde_json::to_value(&result) {
                let tokens = (prompt.len() / 4) as u64;
                store.put_with_tokens(cache_key, MemoValue::Json(json), tokens);
            }
        }

        result
    }

    fn name(&self) -> &'static str {
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
        matches!(
            complexity,
            QueryComplexity::Medium | QueryComplexity::Complex
        )
    }
}
