// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM-based tree navigation retriever.
//!
//! This retriever uses an LLM to navigate the document tree level by level,
//! finding the most relevant content for a given query.

use async_trait::async_trait;
use tracing::{debug, info, warn};

use crate::config::RetrievalConfig;
use crate::core::{DocumentTree, NodeId, Result, Error, Retriever};

use super::{RetrieveOptions, RetrievalResult, NavigationDecision};

/// LLM-based tree navigation retriever.
///
/// This retriever navigates the document tree using an LLM to decide
/// which branches to explore at each level.
#[derive(Debug)]
pub struct LlmNavigator {
    /// Retrieval configuration.
    config: RetrievalConfig,
}

impl LlmNavigator {
    /// Create a new LLM navigator.
    pub fn new(config: RetrievalConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(RetrievalConfig::default())
    }

    /// Navigate the tree to find relevant nodes.
    pub async fn navigate(
        &self,
        tree: &DocumentTree,
        query: &str,
        options: &RetrieveOptions,
    ) -> Result<Vec<RetrievalResult>> {
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
        tree: &DocumentTree,
        node_id: NodeId,
        query: &str,
        options: &RetrieveOptions,
        results: &mut Vec<RetrievalResult>,
        visited: &mut std::collections::HashSet<String>,
        depth: usize,
    ) -> Result<()> {
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
            NavigationDecision::ThisIsTheAnswer => {
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
            NavigationDecision::GoToChild(idx) => {
                if idx < children.len() {
                    // Explore the selected child
                    Box::pin(self.navigate_from_node(
                        tree, children[idx], query, options, results, visited, depth + 1
                    )).await?;
                }
            }
            NavigationDecision::ExploreMore => {
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
    ) -> Result<NavigationDecision> {
        if child_info.is_empty() {
            return Ok(NavigationDecision::ThisIsTheAnswer);
        }

        // Check if API key is configured
        let api_key = match &self.config.api_key {
            Some(key) if !key.is_empty() => key,
            _ => {
                // Fallback to keyword matching if no API key
                debug!("No API key configured, using keyword matching");
                return self.keyword_navigation(query, child_info).await;
            }
        };

        // Build prompt for LLM
        let prompt = self.build_navigation_prompt(query, current_title, child_info);

        // Call LLM
        match self.call_llm(api_key, &prompt).await {
            Ok(response) => {
                self.parse_navigation_response(&response, child_info.len())
            }
            Err(e) => {
                warn!("LLM call failed: {}, falling back to keyword matching", e);
                self.keyword_navigation(query, child_info).await
            }
        }
    }

    /// Build the navigation prompt for the LLM.
    fn build_navigation_prompt(
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
            r#"You are a document navigation assistant. Given a user query and available document sections, decide which section to explore.

User Query: {}

Current Section: {}

Available Subsections:
{}

Instructions:
- If the current section directly answers the query, respond with "0"
- If a subsection is more relevant, respond with the subsection number (1-{})
- If multiple sections might be relevant, respond with "all"
- Respond with ONLY the number or "all", nothing else."#,
            query,
            current_title,
            sections,
            child_info.len()
        )
    }

    /// Call the LLM API.
    async fn call_llm(&self, api_key: &str, prompt: &str) -> Result<String> {
        use async_openai::{
            types::completions::CreateCompletionRequestArgs,
            Client,
            config::OpenAIConfig,
        };

        let openai_config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(&self.config.endpoint);

        let client = Client::with_config(openai_config);

        let request = CreateCompletionRequestArgs::default()
            .model(&self.config.model)
            .prompt(prompt)
            .max_tokens(10u16)  // We only need a short response
            .temperature(self.config.temperature)
            .build()
            .map_err(|e| Error::Retrieval(format!("Failed to build request: {}", e)))?;

        let response = client.completions().create(request).await
            .map_err(|e| Error::Retrieval(format!("LLM API error: {}", e)))?;

        let content = response
            .choices
            .first()
            .map(|c| c.text.trim().to_string())
            .unwrap_or_default();

        debug!("LLM response: {}", content);
        Ok(content)
    }

    /// Parse the LLM navigation response.
    fn parse_navigation_response(&self, response: &str, num_children: usize) -> Result<NavigationDecision> {
        let response = response.trim().to_lowercase();

        if response == "0" || response == "current" {
            return Ok(NavigationDecision::ThisIsTheAnswer);
        }

        if response == "all" || response == "multiple" {
            return Ok(NavigationDecision::ExploreMore);
        }

        // Try to parse as a number
        if let Ok(num) = response.parse::<usize>() {
            if num == 0 {
                return Ok(NavigationDecision::ThisIsTheAnswer);
            }
            if num <= num_children {
                return Ok(NavigationDecision::GoToChild(num - 1));  // Convert to 0-indexed
            }
        }

        // Default to exploring all
        Ok(NavigationDecision::ExploreMore)
    }

    /// Fallback keyword-based navigation.
    async fn keyword_navigation(&self, query: &str, child_info: &[String]) -> Result<NavigationDecision> {
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
            Ok(NavigationDecision::GoToChild(best_match.0))
        } else {
            Ok(NavigationDecision::ExploreMore)
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
        tree: &DocumentTree,
        query: &str,
        options: &RetrieveOptions,
    ) -> Result<Vec<String>> {
        info!("Retrieving content for query: {}", query);

        let results = self.navigate(tree, query, options).await?;

        // Convert results to content strings
        let contents: Vec<String> = results
            .into_iter()
            .filter_map(|r| {
                if r.content.is_some() || r.summary.is_some() {
                    let mut text = format!("## {}\n", r.title);
                    if let Some(summary) = &r.summary {
                        text.push_str(&format!("Summary: {}\n", summary));
                    }
                    if let Some(content) = &r.content {
                        text.push_str(&format!("\n{}\n", content));
                    }
                    Some(text)
                } else {
                    None
                }
            })
            .collect();

        info!("Retrieved {} results", contents.len());
        Ok(contents)
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
        assert!(matches!(decision, NavigationDecision::GoToChild(0)));
    }
}
