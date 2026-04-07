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
//!
//! # Context Modes
//!
//! The builder supports different verbosity levels:
//! - [`Full`](ContextMode::Full): Complete context with all details
//! - [`Summary`](ContextMode::Summary): Titles and summaries only (default)
//! - [`Minimal`](ContextMode::Minimal): Minimal context for token efficiency
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::retrieval::pilot::builder::{ContextBuilder, ContextMode};
//!
//! // Summary mode (default) - token efficient
//! let builder = ContextBuilder::new(500)
//!     .with_mode(ContextMode::Summary);
//!
//! // Full mode - maximum context
//! let builder = ContextBuilder::new(1000)
//!     .with_mode(ContextMode::Full);
//!
//! // Minimal mode - ultra efficient
//! let builder = ContextBuilder::new(200)
//!     .with_mode(ContextMode::Minimal);
//! ```

use std::collections::HashSet;

use super::SearchState;
use crate::document::{DocumentTree, NodeId};

/// Context verbosity mode for LLM calls.
///
/// Controls how much detail is included in the context sent to the LLM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextMode {
    /// Full context with all details.
    ///
    /// - Includes complete content for current node
    /// - Full summaries for all candidates
    /// - Complete TOC with summaries
    ///
    /// Use when accuracy is more important than token cost.
    Full,

    /// Summary mode with titles and summaries only (default).
    ///
    /// - Only titles for path
    /// - Titles + short summaries for candidates
    /// - TOC with titles only
    ///
    /// Best balance of context and token efficiency.
    #[default]
    Summary,

    /// Minimal context for maximum token efficiency.
    ///
    /// - Only essential path info
    /// - Top candidates with titles only
    /// - Abbreviated TOC
    ///
    /// Use when token budget is very tight.
    Minimal,
}

impl ContextMode {
    /// Get the default token budget for this mode.
    pub fn default_token_budget(&self) -> usize {
        match self {
            ContextMode::Full => 1000,
            ContextMode::Summary => 500,
            ContextMode::Minimal => 200,
        }
    }

    /// Get the maximum depth for TOC traversal.
    pub fn max_toc_depth(&self) -> usize {
        match self {
            ContextMode::Full => 5,
            ContextMode::Summary => 3,
            ContextMode::Minimal => 2,
        }
    }

    /// Get the maximum number of candidates to include.
    pub fn max_candidates(&self) -> usize {
        match self {
            ContextMode::Full => 15,
            ContextMode::Summary => 10,
            ContextMode::Minimal => 5,
        }
    }

    /// Check if summaries should be included for candidates.
    pub fn include_summaries(&self) -> bool {
        match self {
            ContextMode::Full => true,
            ContextMode::Summary => true,
            ContextMode::Minimal => false,
        }
    }

    /// Get the summary truncation length (in characters).
    pub fn summary_truncation(&self) -> usize {
        match self {
            ContextMode::Full => 500,
            ContextMode::Summary => 150,
            ContextMode::Minimal => 50,
        }
    }
}

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
    pub fn with_distribution(
        total: usize,
        query_pct: f32,
        path_pct: f32,
        candidates_pct: f32,
        siblings_pct: f32,
    ) -> Self {
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
            self.query_section, self.path_section, self.candidates_section, self.toc_section
        )
    }

    /// Check if context is empty.
    pub fn is_empty(&self) -> bool {
        self.query_section.is_empty()
            && self.path_section.is_empty()
            && self.candidates_section.is_empty()
    }

    /// Get a hash of the query for feedback learning.
    pub fn query_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.query_section.hash(&mut hasher);
        hasher.finish()
    }

    /// Get a hash of the path for feedback learning.
    pub fn path_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.path_section.hash(&mut hasher);
        hasher.finish()
    }
}

/// Context builder for Pilot LLM calls.
///
/// Builds structured context from search state, optimized for
/// token efficiency while providing enough information for
/// good LLM decisions.
///
/// # Context Modes
///
/// The builder supports different verbosity levels:
/// - [`ContextMode::Full`]: Complete context with all details
/// - [`ContextMode::Summary`]: Titles and summaries only (default)
/// - [`ContextMode::Minimal`]: Minimal context for token efficiency
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::pilot::builder::{ContextBuilder, ContextMode};
///
/// // Default summary mode
/// let builder = ContextBuilder::new(500);
/// let context = builder.build(&state);
///
/// // Full mode for maximum context
/// let builder = ContextBuilder::new(1000).with_mode(ContextMode::Full);
///
/// // Minimal mode for tight token budgets
/// let builder = ContextBuilder::new(200).with_mode(ContextMode::Minimal);
/// ```
pub struct ContextBuilder {
    /// Token budget for context.
    budget: TokenBudget,
    /// Context verbosity mode.
    mode: ContextMode,
    /// Maximum candidates to include (overrides mode default).
    max_candidates: Option<usize>,
    /// Maximum path depth to show (overrides mode default).
    max_path_depth: Option<usize>,
    /// Whether to include summaries for candidates (overrides mode default).
    include_summaries: Option<bool>,
    /// Maximum TOC depth (overrides mode default).
    max_toc_depth: Option<usize>,
    /// Summary truncation length (overrides mode default).
    summary_truncation: Option<usize>,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new(500)
    }
}

impl ContextBuilder {
    /// Create a new context builder with the given token budget.
    ///
    /// Uses [`ContextMode::Summary`] by default.
    pub fn new(token_budget: usize) -> Self {
        Self {
            budget: TokenBudget::new(token_budget),
            mode: ContextMode::default(),
            max_candidates: None,
            max_path_depth: None,
            include_summaries: None,
            max_toc_depth: None,
            summary_truncation: None,
        }
    }

    /// Create with custom budget object.
    pub fn with_budget(budget: TokenBudget) -> Self {
        Self {
            budget,
            mode: ContextMode::default(),
            max_candidates: None,
            max_path_depth: None,
            include_summaries: None,
            max_toc_depth: None,
            summary_truncation: None,
        }
    }

    /// Set the context mode.
    ///
    /// This controls the verbosity of the context:
    /// - `Full`: Complete context with all details
    /// - `Summary`: Titles and summaries only (default)
    /// - `Minimal`: Minimal context for token efficiency
    pub fn with_mode(mut self, mode: ContextMode) -> Self {
        self.mode = mode;
        // Update budget if not explicitly set
        if self.budget.total < mode.default_token_budget() {
            self.budget = TokenBudget::new(mode.default_token_budget());
        }
        self
    }

    /// Set maximum candidates to include (overrides mode default).
    pub fn with_max_candidates(mut self, max: usize) -> Self {
        self.max_candidates = Some(max);
        self
    }

    /// Set maximum path depth to show (overrides mode default).
    pub fn with_max_path_depth(mut self, max: usize) -> Self {
        self.max_path_depth = Some(max);
        self
    }

    /// Set whether to include summaries for candidates (overrides mode default).
    pub fn with_summaries(mut self, include: bool) -> Self {
        self.include_summaries = Some(include);
        self
    }

    /// Set maximum TOC depth (overrides mode default).
    pub fn with_max_toc_depth(mut self, depth: usize) -> Self {
        self.max_toc_depth = Some(depth);
        self
    }

    /// Set summary truncation length (overrides mode default).
    pub fn with_summary_truncation(mut self, len: usize) -> Self {
        self.summary_truncation = Some(len);
        self
    }

    /// Get the effective max candidates (mode default or override).
    fn effective_max_candidates(&self) -> usize {
        self.max_candidates.unwrap_or_else(|| self.mode.max_candidates())
    }

    /// Get the effective max path depth (mode default or override).
    fn effective_max_path_depth(&self) -> usize {
        self.max_path_depth.unwrap_or(5)
    }

    /// Get the effective include summaries setting (mode default or override).
    fn effective_include_summaries(&self) -> bool {
        self.include_summaries.unwrap_or_else(|| self.mode.include_summaries())
    }

    /// Get the effective max TOC depth (mode default or override).
    fn effective_max_toc_depth(&self) -> usize {
        self.max_toc_depth.unwrap_or_else(|| self.mode.max_toc_depth())
    }

    /// Get the effective summary truncation length (mode default or override).
    fn effective_summary_truncation(&self) -> usize {
        self.summary_truncation.unwrap_or_else(|| self.mode.summary_truncation())
    }

    /// Get the current mode.
    pub fn mode(&self) -> ContextMode {
        self.mode
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
        ctx.path_section = format!(
            "Failed path:\n{}",
            self.build_path_section(state.tree, failed_path)
        );
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
        let max_depth = self.effective_max_path_depth();
        let start = if path.len() > max_depth {
            path.len() - max_depth
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

    /// Build candidates section with dynamic truncation.
    fn build_candidates_section(&self, tree: &DocumentTree, candidates: &[NodeId]) -> String {
        if candidates.is_empty() {
            return "Candidates: (none)\n".to_string();
        }

        let mut result = String::from("Candidate Nodes:\n");
        let mut tokens_used = 0;
        let max_tokens = self.budget.candidates;
        let max_candidates = self.effective_max_candidates();
        let include_summaries = self.effective_include_summaries();
        let summary_trunc = self.effective_summary_truncation();

        for (i, node_id) in candidates.iter().take(max_candidates).enumerate() {
            if tokens_used >= max_tokens {
                result.push_str("... (more candidates omitted)\n");
                break;
            }

            if let Some(node) = tree.get(*node_id) {
                let entry = if include_summaries && !node.summary.is_empty() {
                    let truncated_summary = self.truncate_text(&node.summary, summary_trunc);
                    format!("{}. {} [{}]\n", i + 1, node.title, truncated_summary)
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
                let marker = if *sibling_id == current_id {
                    "⭐ "
                } else {
                    ""
                };
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
        let max_depth = self.effective_max_toc_depth();
        let include_summaries = self.effective_include_summaries();
        let summary_trunc = self.effective_summary_truncation();

        self.build_toc_recursive(
            tree,
            tree.root(),
            0,
            &mut result,
            &mut tokens_used,
            max_tokens,
            max_depth,
            include_summaries,
            summary_trunc,
        );

        result
    }

    /// Recursive helper for building TOC.
    fn build_toc_recursive(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        depth: usize,
        result: &mut String,
        tokens_used: &mut usize,
        max_tokens: usize,
        max_depth: usize,
        include_summaries: bool,
        summary_trunc: usize,
    ) {
        if *tokens_used >= max_tokens || depth > max_depth {
            return;
        }

        if let Some(node) = tree.get(node_id) {
            let indent = "  ".repeat(depth);
            let entry = if include_summaries && !node.summary.is_empty() && depth < 2 {
                let truncated = self.truncate_text(&node.summary, summary_trunc);
                format!("{}{} [{}]\n", indent, node.title, truncated)
            } else {
                format!("{}{}\n", indent, node.title)
            };
            *tokens_used += entry.len() / 4; // Rough estimate
            result.push_str(&entry);

            // Only show children for first few levels
            if depth < max_depth {
                for child_id in tree.children(node_id) {
                    self.build_toc_recursive(
                        tree,
                        child_id,
                        depth + 1,
                        result,
                        tokens_used,
                        max_tokens,
                        max_depth,
                        include_summaries,
                        summary_trunc,
                    );
                }
            }
        }
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

    /// Truncate text to a maximum character length.
    ///
    /// Adds "..." if truncation occurs.
    fn truncate_text(&self, text: &str, max_chars: usize) -> String {
        if text.chars().count() <= max_chars {
            text.to_string()
        } else {
            let truncated: String = text.chars().take(max_chars).collect();
            // Try to break at word boundary
            if let Some(last_space) = truncated.rfind(' ') {
                if last_space > max_chars / 2 {
                    format!("{}...", &truncated[..last_space])
                } else {
                    format!("{}...", truncated)
                }
            } else {
                format!("{}...", truncated)
            }
        }
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
        let root = arena.new_node(crate::document::TreeNode {
            title: "Root".to_string(),
            content: "Root content".to_string(),
            summary: "Root summary".to_string(),
            depth: 0,
            ..Default::default()
        });

        let child1 = arena.new_node(crate::document::TreeNode {
            title: "Configuration".to_string(),
            content: "Config content".to_string(),
            summary: "Configuration options".to_string(),
            depth: 1,
            ..Default::default()
        });

        let child2 = arena.new_node(crate::document::TreeNode {
            title: "API Reference".to_string(),
            content: "API content".to_string(),
            summary: "API documentation".to_string(),
            depth: 1,
            ..Default::default()
        });

        root.append(child1, &mut arena);
        root.append(child2, &mut arena);

        DocumentTree::from_raw(arena, crate::document::NodeId(root))
    }

    #[test]
    fn test_token_budget_distribution() {
        let budget = TokenBudget::new(500);
        assert_eq!(budget.query, 150); // 30%
        assert_eq!(budget.path, 100); // 20%
        assert_eq!(budget.candidates, 200); // 40%
        assert_eq!(budget.siblings, 50); // 10%
    }

    #[test]
    fn test_context_builder_creation() {
        let builder = ContextBuilder::new(500);
        assert_eq!(builder.effective_max_candidates(), 10); // Default from Summary mode
        assert_eq!(builder.effective_max_path_depth(), 5);
        assert!(builder.effective_include_summaries());
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
        assert!(
            result.contains("..."),
            "Expected truncation, got: {}",
            result
        );
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
