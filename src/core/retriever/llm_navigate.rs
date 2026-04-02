// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM-based tree navigation retriever.
//!
//! This retriever uses an LLM to navigate the document tree level by level,
//! finding the most relevant content for a given query.

use async_trait::async_trait;
use tracing::{debug, info, warn};

use crate::config::RetrievalConfig;
use crate::core::{NodeId, VectorlessTree};
use crate::llm::LlmClient;

use super::retriever::{Retriever, RetrieverResult};
use super::types::{RetrieveOptions, RetrieveResponse, RetrievalResult, QueryComplexity};

/// Internal navigation decision from LLM.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NavDecision {
    /// Go to the specified child node.
    GoToChild(usize),
    /// The current node is the answer.
    ThisIsTheAnswer,
    /// Need to explore more at this level.
    ExploreMore,
}

/// LLM-based tree navigation retriever.
///
/// This retriever navigates the document tree using an LLM to decide
/// which branches to explore at each level.
#[derive(Debug)]
pub struct LlmNavigator {
    /// Retrieval configuration.
    config: RetrievalConfig,
    /// LLM client.
    client: LlmClient,
}

impl LlmNavigator {
    /// Create a new LLM navigator.
    pub fn new(config: RetrievalConfig) -> Self {
        let client = LlmClient::new(config.clone().into());
        Self { config, client }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(RetrievalConfig::default())
    }

    /// Navigate the tree to find relevant nodes.
    pub async fn navigate(
        &self,
        tree: &VectorlessTree,
        query: &str,
        options: &RetrieveOptions,
    ) -> RetrieverResult<Vec<RetrievalResult>> {
        let mut results = Vec::new();
        let mut visited = std::collections::HashSet::new();

        // Start from root
        let root = tree.root();

        // BFS-style exploration with LLM guidance
        self.navigate_from_node(tree, root, query, options, &mut results, &mut visited, 0)
            .await?;

        // Sort by score and limit to top_k
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(options.top_k);

        Ok(results)
    }

    async fn navigate_from_node(
        &self,
        tree: &VectorlessTree,
        node_id: NodeId,
        query: &str,
        options: &RetrieveOptions,
        results: &mut Vec<RetrievalResult>,
        visited: &mut std::collections::HashSet<String>,
        depth: usize,
    ) -> RetrieverResult<()> {
        // Prevent cycles using node_id string representation
        let node_key = format!("{:?}", node_id);
        if visited.contains(&node_key) {
            return Ok(());
        }
        visited.insert(node_key.clone());

        // Get current node
        let node = match tree.get(node_id) {
            Some(n) => n,
            None => return Ok(()),
        };

        debug!("Navigating at depth {}: {}", depth, node.title);

        // If this is a leaf node, add it as a result
        if tree.is_leaf(node_id) {
            let score = self.compute_relevance_score(query, &node.title, &node.content);

            let result = RetrievalResult::new(&node.title)
                .with_node_id(node.node_id.clone().unwrap_or_default())
                .with_depth(depth)
                .with_score(score);

            let mut result = if options.include_content {
                result.with_content(&node.content)
            } else {
                result
            };

            if options.include_summaries && !node.summary.is_empty() {
                result = result.with_summary(&node.summary);
            }

            if let (Some(start), Some(end)) = (node.start_page, node.end_page) {
                result = result.with_page_range(start, end);
            }

            results.push(result);
            return Ok(());
        }

        // Get children
        let children = tree.children(node_id);
        if children.is_empty() {
            return Ok(());
        }

        // Build navigation context
        let child_info: Vec<String> = children
            .iter()
            .filter_map(|&child_id| {
                tree.get(child_id).map(|child| {
                    let summary = if child.summary.is_empty() {
                        &child.content
                    } else {
                        &child.summary
                    };
                    let truncated = if summary.len() > 200 {
                        format!("{}...", &summary[..200])
                    } else {
                        summary.to_string()
                    };
                    format!(
                        "{}: {}",
                        child.title,
                        truncated.trim()
                    )
                })
            })
            .collect();

        // Use LLM to decide which child to explore
        let decision = self.make_navigation_decision(query, &node.title, &child_info).await?;

        match decision {
            NavDecision::ThisIsTheAnswer => {
                // Current node is relevant, add it
                let score = self.compute_relevance_score(query, &node.title, &node.content);

                let result = RetrievalResult::new(&node.title)
                    .with_node_id(node.node_id.clone().unwrap_or_default())
                    .with_depth(depth)
                    .with_score(score);

                let mut result = if options.include_content {
                    result.with_content(&node.content)
                } else {
                    result
                };

                if options.include_summaries && !node.summary.is_empty() {
                    result = result.with_summary(&node.summary);
                }

                results.push(result);
            }
            NavDecision::GoToChild(idx) => {
                if idx < children.len() {
                    // Explore the selected child
                    Box::pin(self.navigate_from_node(
                        tree, children[idx], query, options, results, visited, depth + 1
                    )).await?;
                }
            }
            NavDecision::ExploreMore => {
                // Explore multiple children that might be relevant
                for child_id in children {
                    Box::pin(self.navigate_from_node(
                        tree, child_id, query, options, results, visited, depth + 1
                    )).await?;

                    // Limit exploration
                    if results.len() >= options.top_k * 2 {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Use LLM to make a navigation decision.
    async fn make_navigation_decision(
        &self,
        query: &str,
        current_title: &str,
        child_info: &[String],
    ) -> RetrieverResult<NavDecision> {
        if child_info.is_empty() {
            return Ok(NavDecision::ThisIsTheAnswer);
        }

        // Check if API key is configured
        let has_api_key = self.config.api_key.is_some()
            || std::env::var("OPENAI_API_KEY").is_ok()
            || std::env::var("ANTHROPIC_API_KEY").is_ok();

        if !has_api_key {
            // Fallback to keyword matching if no API key
            debug!("No API key configured, using keyword matching");
            return self.keyword_navigation(query, child_info).await;
        }

        // Build prompts for LLM
        let system_prompt = self.build_system_prompt();
        let user_prompt = self.build_user_prompt(query, current_title, child_info);

        // Call LLM using unified client
        match self.client.complete(&system_prompt, &user_prompt).await {
            Ok(response) => {
                self.parse_navigation_response(&response, child_info.len())
            }
            Err(e) => {
                warn!("LLM call failed: {}, falling back to keyword matching", e);
                self.keyword_navigation(query, child_info).await
            }
        }
    }

    /// Build the system prompt for navigation.
    fn build_system_prompt(&self) -> String {
        "You are a document navigation assistant. \
         Given a user query and available document sections, decide which section to explore. \
         Respond with ONLY a single number (0 for current section, 1-N for subsection) or \"all\". \
         Do not include any explanation or additional text.".to_string()
    }

    /// Build the user prompt for navigation (actual content).
    fn build_user_prompt(
        &self,
        query: &str,
        current_title: &str,
        child_info: &[String],
    ) -> String {
        let sections = child_info.iter()
            .enumerate()
            .map(|(i, info)| format!("{}. {}", i + 1, info))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"User Query: {}

Current Section: {}

Available Subsections:
{}

Instructions:
- If the current section directly answers the query, respond with "0"
- If a subsection is more relevant, respond with the subsection number (1-{})
- If multiple sections might be relevant, respond with "all""#,
            query,
            current_title,
            sections,
            child_info.len()
        )
    }

    /// Parse the LLM navigation response.
    fn parse_navigation_response(&self, response: &str, num_children: usize) -> RetrieverResult<NavDecision> {
        let response = response.trim().to_lowercase();

        if response == "0" || response == "current" {
            return Ok(NavDecision::ThisIsTheAnswer);
        }

        if response == "all" || response == "multiple" {
            return Ok(NavDecision::ExploreMore);
        }

        // Try to parse as a number
        if let Ok(num) = response.parse::<usize>() {
            if num == 0 {
                return Ok(NavDecision::ThisIsTheAnswer);
            }
            if num <= num_children {
                return Ok(NavDecision::GoToChild(num - 1));  // Convert to 0-indexed
            }
        }

        // Default to exploring all
        Ok(NavDecision::ExploreMore)
    }

    /// Fallback keyword-based navigation.
    async fn keyword_navigation(&self, query: &str, child_info: &[String]) -> RetrieverResult<NavDecision> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut best_match = (0usize, 0usize);  // (index, score)

        for (i, info) in child_info.iter().enumerate() {
            let info_lower = info.to_lowercase();
            let overlap = query_words
                .iter()
                .filter(|word| info_lower.contains(*word))
                .count();

            if overlap > best_match.1 {
                best_match = (i, overlap);
            }
        }

        if best_match.1 > 0 {
            Ok(NavDecision::GoToChild(best_match.0))
        } else {
            Ok(NavDecision::ExploreMore)
        }
    }

    /// Compute relevance score for a node.
    fn compute_relevance_score(&self, query: &str, title: &str, content: &str) -> f32 {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        if query_words.is_empty() {
            return 0.5;
        }

        let title_lower = title.to_lowercase();
        let content_lower = content.to_lowercase();

        let mut title_matches = 0;
        let mut content_matches = 0;

        for word in &query_words {
            if title_lower.contains(word) {
                title_matches += 1;
            }
            if content_lower.contains(word) {
                content_matches += 1;
            }
        }

        // Weighted score
        let title_score = title_matches as f32 / query_words.len() as f32;
        let content_score = content_matches as f32 / query_words.len() as f32;

        (title_score * 0.6 + content_score * 0.4).max(0.1)
    }
}

#[async_trait]
impl Retriever for LlmNavigator {
    async fn retrieve(
        &self,
        tree: &VectorlessTree,
        query: &str,
        options: &RetrieveOptions,
    ) -> RetrieverResult<RetrieveResponse> {
        info!("Retrieving content for query: {}", query);
        let results = self.navigate(tree, query, options).await?;
        info!("Retrieved {} results", results.len());

        // Build response
        let content = results
            .iter()
            .filter_map(|r| r.content.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n");

        let confidence = if results.is_empty() {
            0.0
        } else {
            results.iter().map(|r| r.score).sum::<f32>() / results.len() as f32
        };

        Ok(RetrieveResponse {
            results,
            content,
            confidence,
            is_sufficient: confidence > 0.5,
            strategy_used: "llm_navigate".to_string(),
            complexity: QueryComplexity::Medium,
            trace: Vec::new(),
            tokens_used: 0,
        })
    }

    fn name(&self) -> &str {
        "llm_navigator"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigator_creation() {
        let navigator = LlmNavigator::with_defaults();
        assert!(navigator.config.model.len() > 0);
    }

    #[test]
    fn test_relevance_score() {
        let navigator = LlmNavigator::with_defaults();

        let score = navigator.compute_relevance_score(
            "tree structure",
            "Document Tree Structure",
            "This describes the tree structure."
        );
        assert!(score > 0.5);

        let score = navigator.compute_relevance_score(
            "unrelated query",
            "Different Topic",
            "Completely different content."
        );
        assert!(score < 0.5);
    }

    #[tokio::test]
    async fn test_keyword_navigation() {
        let navigator = LlmNavigator::with_defaults();

        let child_info = vec![
            "Tree Structure: How trees are organized".to_string(),
            "Configuration: Settings and options".to_string(),
        ];

        let decision = navigator.keyword_navigation("tree organization", &child_info).await.unwrap();
        assert!(matches!(decision, NavDecision::GoToChild(0)));
    }
}
