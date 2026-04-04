// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Context builder for Pilot LLM calls.
//!
//! Constructs the context information sent to the LLM, including:
//! - Current path in the document tree
//! - Candidate nodes with their summaries
//! - TOC view for navigation context
//!
//! Token budget is distributed across components:
//! - Query: 30%
//! - Current path: 20%
//! - Candidates: 40%
//! - Sibling context: 10%

use std::collections::HashSet;

use crate::domain::{DocumentTree, NodeId};
use super::SearchState;

/// Token budget distribution for context building.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    /// Total tokens available.
    pub total: usize,
    /// Tokens for query section.
    pub query: usize,
    /// Tokens for current path.
    pub path: usize,
    /// Tokens for candidates.
    pub candidates: usize,
    /// Tokens for sibling context.
    pub siblings: usize,
}

impl TokenBudget {
    /// Create a new token budget with the given total.
    pub fn new(total: usize) -> Self {
        Self {
            total,
            query: (total as f32 * 0.30) as usize,
            path: (total as f32 * 0.20) as usize,
            candidates: (total as f32 * 0.40) as usize,
            siblings: (total as f32 * 0.10) as usize,
        }
    }

    /// Create budget with custom distribution.
    pub fn with_distribution(total: usize, query_pct: f32, path_pct: f32, candidates_pct: f32, siblings_pct: f32) -> Self {
        let sum = query_pct + path_pct + candidates_pct + siblings_pct;
        Self {
            total,
            query: (total as f32 * query_pct / sum) as usize,
            path: (total as f32 * path_pct / sum) as usize,
            candidates: (total as f32 * candidates_pct / sum) as usize,
            siblings: (total as f32 * siblings_pct / sum) as usize,
        }
    }
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self::new(500)
    }
}

/// Built context for LLM call.
#[derive(Debug, Clone, Default)]
pub struct PilotContext {
    /// Formatted query section.
    pub query_section: String,
    /// Formatted current path.
    pub path_section: String,
    /// Formatted candidates section.
    pub candidates_section: String,
    /// Formatted TOC/sibling context.
    pub toc_section: String,
    /// Estimated total tokens.
    pub estimated_tokens: usize,
}

impl PilotContext {
    /// Get the full context as a single string.
    pub fn to_string(&self) -> String {
        format!(
            "{}\n{}\n{}\n{}",
            self.query_section,
            self.path_section,
            self.candidates_section,
            self.toc_section
        )
    }

    /// Check if context is empty.
    pub fn is_empty(&self) -> bool {
        self.query_section.is_empty()
            && self.path_section.is_empty()
            && self.candidates_section.is_empty()
    }
}

/// Context builder for Pilot LLM calls.
///
/// Builds structured context from search state, optimized for
/// token efficiency while providing enough information for
/// good LLM decisions.
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::pilot::ContextBuilder;
///
/// let builder = ContextBuilder::new(500);
/// let context = builder.build(&state, &tree);
/// println!("Estimated tokens: {}", context.estimated_tokens);
/// ```
pub struct ContextBuilder {
    /// Token budget for context.
    budget: TokenBudget,
    /// Maximum candidates to include.
    max_candidates: usize,
    /// Maximum path depth to show.
    max_path_depth: usize,
    /// Whether to include summaries for candidates.
    include_summaries: bool,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new(500)
    }
}

impl ContextBuilder {
    /// Create a new context builder with the given token budget.
    pub fn new(token_budget: usize) -> Self {
        Self {
            budget: TokenBudget::new(token_budget),
            max_candidates: 10,
            max_path_depth: 5,
            include_summaries: true,
        }
    }

    /// Create with custom budget object.
    pub fn with_budget(budget: TokenBudget) -> Self {
        Self {
            budget,
            max_candidates: 10,
            max_path_depth: 5,
            include_summaries: true,
        }
    }

    /// Set maximum candidates to include.
    pub fn with_max_candidates(mut self, max: usize) -> Self {
        self.max_candidates = max;
        self
    }

    /// Set maximum path depth to show.
    pub fn with_max_path_depth(mut self, max: usize) -> Self {
        self.max_path_depth = max;
        self
    }

    /// Set whether to include summaries for candidates.
    pub fn with_summaries(mut self, include: bool) -> Self {
        self.include_summaries = include;
        self
    }

    /// Build context from search state.
    pub fn build(&self, state: &SearchState<'_>) -> PilotContext {
        let mut ctx = PilotContext::default();

        // Build query section
        ctx.query_section = self.build_query_section(state.query);
        ctx.estimated_tokens += self.estimate_tokens(&ctx.query_section);

        // Build path section
        ctx.path_section = self.build_path_section(state.tree, state.path);
        ctx.estimated_tokens += self.estimate_tokens(&ctx.path_section);

        // Build candidates section
        ctx.candidates_section = self.build_candidates_section(state.tree, state.candidates);
        ctx.estimated_tokens += self.estimate_tokens(&ctx.candidates_section);

        // Build TOC section (siblings context)
        ctx.toc_section = self.build_toc_section(state.tree, state.path);
        ctx.estimated_tokens += self.estimate_tokens(&ctx.toc_section);

        ctx
    }

    /// Build context for START intervention point.
    pub fn build_start_context(&self, tree: &DocumentTree, query: &str) -> PilotContext {
        let mut ctx = PilotContext::default();

        // Build query section
        ctx.query_section = self.build_query_section(query);
        ctx.estimated_tokens += self.estimate_tokens(&ctx.query_section);

        // Build full TOC for start
        ctx.toc_section = self.build_full_toc(tree);
        ctx.estimated_tokens += self.estimate_tokens(&ctx.toc_section);

        ctx
    }

    /// Build context for BACKTRACK intervention point.
    pub fn build_backtrack_context(
        &self,
        state: &SearchState<'_>,
        failed_path: &[NodeId],
    ) -> PilotContext {
        let mut ctx = PilotContext::default();

        // Build query section
        ctx.query_section = self.build_query_section(state.query);
        ctx.estimated_tokens += self.estimate_tokens(&ctx.query_section);

        // Show failed path
        ctx.path_section = format!("Failed path:\n{}", self.build_path_section(state.tree, failed_path));
        ctx.estimated_tokens += self.estimate_tokens(&ctx.path_section);

        // Show unvisited alternatives
        ctx.candidates_section = self.build_unvisited_section(state.tree, state.visited);
        ctx.estimated_tokens += self.estimate_tokens(&ctx.candidates_section);

        ctx
    }

    /// Build query section.
    fn build_query_section(&self, query: &str) -> String {
        // Truncate if needed
        let truncated = if query.chars().count() > self.budget.query * 4 {
            let chars: Vec<char> = query.chars().take(self.budget.query * 4).collect();
            format!("{}...", chars.into_iter().collect::<String>())
        } else {
            query.to_string()
        };

        format!("User Query:\n{}\n", truncated)
    }

    /// Build current path section.
    fn build_path_section(&self, tree: &DocumentTree, path: &[NodeId]) -> String {
        if path.is_empty() {
            return "Current Position: Root\n".to_string();
        }

        let mut result = String::from("Current Path:\n");
        result.push_str("Root");

        // Limit depth shown
        let start = if path.len() > self.max_path_depth {
            path.len() - self.max_path_depth
        } else {
            0
        };

        if start > 0 {
            result.push_str(" → ...");
        }

        for node_id in path.iter().skip(start) {
            if let Some(node) = tree.get(*node_id) {
                result.push_str(" → ");
                result.push_str(&node.title);
            }
        }

        result.push('\n');
        result
    }

    /// Build candidates section.
    fn build_candidates_section(&self, tree: &DocumentTree, candidates: &[NodeId]) -> String {
        if candidates.is_empty() {
            return "Candidates: (none)\n".to_string();
        }

        let mut result = String::from("Candidate Nodes:\n");
        let mut tokens_used = 0;
        let max_tokens = self.budget.candidates;

        for (i, node_id) in candidates.iter().take(self.max_candidates).enumerate() {
            if tokens_used >= max_tokens {
                result.push_str("... (more candidates omitted)\n");
                break;
            }

            if let Some(node) = tree.get(*node_id) {
                let entry = if self.include_summaries && !node.summary.is_empty() {
                    format!("{}. {} [{}]\n", i + 1, node.title, node.summary)
                } else {
                    format!("{}. {}\n", i + 1, node.title)
                };

                tokens_used += self.estimate_tokens(&entry);
                result.push_str(&entry);
            }
        }

        result
    }

    /// Build TOC section showing siblings.
    fn build_toc_section(&self, tree: &DocumentTree, path: &[NodeId]) -> String {
        if path.is_empty() {
            return String::new();
        }

        // Get parent of current node
        let parent_id = if path.len() >= 2 {
            path[path.len() - 2]
        } else {
            tree.root()
        };

        let siblings = tree.children(parent_id);
        if siblings.len() <= 1 {
            return String::new();
        }

        let current_id = path[path.len() - 1];
        let mut result = String::from("Sibling Context:\n");

        for sibling_id in siblings.iter().take(8) {
            if let Some(node) = tree.get(*sibling_id) {
                let marker = if *sibling_id == current_id { "⭐ " } else { "" };
                result.push_str(&format!("  {}{}\n", marker, node.title));
            }
        }

        result
    }

    /// Build full TOC for start context.
    fn build_full_toc(&self, tree: &DocumentTree) -> String {
        let mut result = String::from("Document Structure:\n");
        let mut tokens_used = 0;
        let max_tokens = self.budget.siblings + self.budget.candidates;

        fn build_toc_recursive(
            tree: &DocumentTree,
            node_id: NodeId,
            depth: usize,
            result: &mut String,
            tokens_used: &mut usize,
            max_tokens: usize,
            max_depth: usize,
        ) {
            if *tokens_used >= max_tokens || depth > max_depth {
                return;
            }

            if let Some(node) = tree.get(node_id) {
                let indent = "  ".repeat(depth);
                let entry = format!("{}{}\n", indent, node.title);
                *tokens_used += entry.len() / 4; // Rough estimate
                result.push_str(&entry);

                // Only show children for first few levels
                if depth < max_depth {
                    for child_id in tree.children(node_id) {
                        build_toc_recursive(tree, child_id, depth + 1, result, tokens_used, max_tokens, max_depth);
                    }
                }
            }
        }

        build_toc_recursive(
            tree,
            tree.root(),
            0,
            &mut result,
            &mut tokens_used,
            max_tokens,
            3, // Max depth to show
        );

        result
    }

    /// Build section showing unvisited nodes.
    fn build_unvisited_section(&self, tree: &DocumentTree, visited: &HashSet<NodeId>) -> String {
        let mut result = String::from("Unvisited Alternatives:\n");
        let mut count = 0;

        // Find unvisited nodes from root's children
        for child_id in tree.children(tree.root()) {
            if !visited.contains(&child_id) {
                if let Some(node) = tree.get(child_id) {
                    result.push_str(&format!("• {} [{}]\n", node.title, node.summary));
                    count += 1;
                    if count >= 5 {
                        break;
                    }
                }
            }
        }

        if count == 0 {
            result.push_str("(all branches explored)\n");
        }

        result
    }

    /// Estimate token count for a string.
    fn estimate_tokens(&self, text: &str) -> usize {
        // Rough estimation: 1 token ≈ 4 chars (English) or 1.5 chars (Chinese)
        let char_count = text.chars().count();
        let chinese_count = text
            .chars()
            .filter(|c| ('\u{4E00}'..='\u{9FFF}').contains(c))
            .count();
        let english_count = char_count - chinese_count;

        (chinese_count as f32 / 1.5 + english_count as f32 / 4.0).ceil() as usize
    }

    /// Get the token budget.
    pub fn budget(&self) -> &TokenBudget {
        &self.budget
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indextree::Arena;

    fn create_test_tree() -> DocumentTree {
        let mut arena = Arena::new();
        let root = arena.new_node(crate::domain::TreeNode {
            title: "Root".to_string(),
            content: "Root content".to_string(),
            summary: "Root summary".to_string(),
            depth: 0,
            ..Default::default()
        });

        let child1 = arena.new_node(crate::domain::TreeNode {
            title: "Configuration".to_string(),
            content: "Config content".to_string(),
            summary: "Configuration options".to_string(),
            depth: 1,
            ..Default::default()
        });

        let child2 = arena.new_node(crate::domain::TreeNode {
            title: "API Reference".to_string(),
            content: "API content".to_string(),
            summary: "API documentation".to_string(),
            depth: 1,
            ..Default::default()
        });

        root.append(child1, &mut arena);
        root.append(child2, &mut arena);

        DocumentTree::from_raw(arena, crate::domain::NodeId(root))
    }

    #[test]
    fn test_token_budget_distribution() {
        let budget = TokenBudget::new(500);
        assert_eq!(budget.query, 150); // 30%
        assert_eq!(budget.path, 100);  // 20%
        assert_eq!(budget.candidates, 200); // 40%
        assert_eq!(budget.siblings, 50); // 10%
    }

    #[test]
    fn test_context_builder_creation() {
        let builder = ContextBuilder::new(500);
        assert_eq!(builder.max_candidates, 10);
        assert_eq!(builder.max_path_depth, 5);
        assert!(builder.include_summaries);
    }

    #[test]
    fn test_build_query_section() {
        let builder = ContextBuilder::new(500);
        let result = builder.build_query_section("How to configure PostgreSQL?");
        assert!(result.contains("How to configure PostgreSQL?"));
        assert!(result.starts_with("User Query:"));
    }

    #[test]
    fn test_build_query_section_truncation() {
        let builder = ContextBuilder::new(20); // Very small budget - 20 * 0.30 = 6 tokens for query = ~24 chars
        let long_query = "This is a very long query that should be truncated because it exceeds the token budget";
        let result = builder.build_query_section(long_query);
        assert!(result.contains("..."), "Expected truncation, got: {}", result);
    }

    #[test]
    fn test_estimate_tokens_english() {
        let builder = ContextBuilder::new(500);
        let text = "Hello world"; // 11 chars ≈ 3 tokens
        let tokens = builder.estimate_tokens(text);
        assert!(tokens >= 2 && tokens <= 4);
    }

    #[test]
    fn test_estimate_tokens_chinese() {
        let builder = ContextBuilder::new(500);
        let text = "这是一个测试"; // 6 chars ≈ 4 tokens
        let tokens = builder.estimate_tokens(text);
        assert!(tokens >= 3 && tokens <= 5);
    }

    #[test]
    fn test_pilot_context_to_string() {
        let ctx = PilotContext {
            query_section: "Query".to_string(),
            path_section: "Path".to_string(),
            candidates_section: "Candidates".to_string(),
            toc_section: "TOC".to_string(),
            estimated_tokens: 100,
        };

        let result = ctx.to_string();
        assert!(result.contains("Query"));
        assert!(result.contains("Path"));
        assert!(result.contains("Candidates"));
        assert!(result.contains("TOC"));
    }

    #[test]
    fn test_pilot_context_is_empty() {
        let empty = PilotContext::default();
        assert!(empty.is_empty());

        let non_empty = PilotContext {
            query_section: "Query".to_string(),
            ..Default::default()
        };
        assert!(!non_empty.is_empty());
    }
}
