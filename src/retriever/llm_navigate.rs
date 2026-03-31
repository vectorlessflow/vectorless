// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM-based tree navigation retriever.
//!
//! This retriever uses an LLM to navigate the document tree level by level,
//! finding the most relevant content for a given query.

use async_trait::async_trait;
use tracing::debug;

use crate::config::RetrievalConfig;
use crate::core::{DocumentTree, NodeId, Result, Retriever};

use super::{RetrieveOptions, RetrievalResult, NavigationDecision};

/// LLM-based tree navigation retriever.
///
/// This retriever navigates the document tree using an LLM to decide
/// which branches to explore at each level.
#[derive(Debug)]
#[allow(dead_code)]
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
        visited: &mut std::collections::HashSet<usize>,
        depth: usize,
    ) -> Result<()> {
        // Prevent cycles
        let node_key = format!("{:?}", node_id);
        if visited.contains(&node_key.len()) {
            return Ok(());
        }
        visited.insert(node_key.len());

        // Get current node
        let node = match tree.get(node_id) {
            Some(n) => n,
            None => return Ok(()),
        };

        debug!("Navigating at depth {}: {}", depth, node.title);

        // If this is a leaf node, add it as a result
        if tree.is_leaf(node_id) {
            let result = RetrievalResult::new(&node.title)
                .with_node_id(node.node_id.clone().unwrap_or_default())
                .with_depth(depth);

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
        let child_summaries: Vec<String> = children
            .iter()
            .filter_map(|&child_id| {
                tree.get(child_id).map(|child| {
                    format!(
                        "[{}] {} {}",
                        child.node_id.as_deref().unwrap_or("?"),
                        child.title,
                        if child.summary.is_empty() {
                            ""
                        } else {
                            &child.summary
                        }
                    )
                })
            })
            .collect();

        // Use LLM to decide which child to explore
        let decision = self.make_navigation_decision(query, &node.title, &child_summaries).await?;

        match decision {
            NavigationDecision::ThisIsTheAnswer => {
                // Current node is relevant, add it
                let result = RetrievalResult::new(&node.title)
                    .with_node_id(node.node_id.clone().unwrap_or_default())
                    .with_depth(depth)
                    .with_score(1.0);

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
        child_summaries: &[String],
    ) -> Result<NavigationDecision> {
        if child_summaries.is_empty() {
            return Ok(NavigationDecision::ThisIsTheAnswer);
        }

        // Build prompt for LLM
        let prompt = format!(
            r#"Given a user query and a list of document sections, decide which section is most relevant.

Query: {}

Current section: {}

Available subsections:
{}

Respond with ONLY the number of the most relevant subsection (1-{}), or '0' if the current section is the answer."#,
            query,
            current_title,
            child_summaries.iter().enumerate()
                .map(|(i, s)| format!("{}. {}", i + 1, s))
                .collect::<Vec<_>>()
                .join("\n"),
            child_summaries.len()
        );

        // For now, implement a simple heuristic
        // TODO: Call actual LLM once we have the API integration ready
        debug!("Navigation prompt: {}", prompt);

        // Simple keyword matching as fallback
        let query_lower = query.to_lowercase();
        for (i, summary) in child_summaries.iter().enumerate() {
            let summary_lower = summary.to_lowercase();
            // Check for keyword overlap
            let overlap = query_lower
                .split_whitespace()
                .filter(|word| summary_lower.contains(word))
                .count();

            if overlap > 0 {
                return Ok(NavigationDecision::GoToChild(i));
            }
        }

        // Default to exploring all
        Ok(NavigationDecision::ExploreMore)
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
}
