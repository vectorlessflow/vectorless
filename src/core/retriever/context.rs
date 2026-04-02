// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Context building for retrieval results.
//!
//! This module provides utilities for building context strings
//! from retrieval results for LLM consumption.

use crate::core::{VectorlessTree, NodeId};
use super::types::RetrievalResult;

/// Context builder for assembling retrieval results.
#[derive(Debug, Default)]
pub struct ContextBuilder {
    /// Maximum tokens for the context.
    max_tokens: usize,

    /// Whether to include titles.
    include_titles: bool,

    /// Whether to include summaries.
    include_summaries: bool,

    /// Whether to include content.
    include_content: bool,

    /// Separator between sections.
    separator: String,
}

impl ContextBuilder {
    /// Create a new context builder.
    pub fn new() -> Self {
        Self {
            max_tokens: 4000,
            include_titles: true,
            include_summaries: true,
            include_content: true,
            separator: "\n\n---\n\n".to_string(),
        }
    }

    /// Set the maximum tokens.
    pub fn with_max_tokens(mut self, tokens: usize) -> Self {
        self.max_tokens = tokens;
        self
    }

    /// Set whether to include titles.
    pub fn with_titles(mut self, include: bool) -> Self {
        self.include_titles = include;
        self
    }

    /// Set whether to include summaries.
    pub fn with_summaries(mut self, include: bool) -> Self {
        self.include_summaries = include;
        self
    }

    /// Set whether to include content.
    pub fn with_content(mut self, include: bool) -> Self {
        self.include_content = include;
        self
    }

    /// Set the separator.
    pub fn with_separator(mut self, separator: impl Into<String>) -> Self {
        self.separator = separator.into();
        self
    }

    /// Build context from retrieval results.
    pub fn build(&self, results: &[RetrievalResult]) -> String {
        let mut sections = Vec::new();
        let mut estimated_tokens = 0;

        for result in results {
            let section = self.format_section(result);

            // Rough token estimation (1 token ≈ 4 characters)
            let section_tokens = section.len() / 4;

            if estimated_tokens + section_tokens > self.max_tokens {
                break;
            }

            estimated_tokens += section_tokens;
            sections.push(section);
        }

        sections.join(&self.separator)
    }

    /// Build context from a document tree starting at a node.
    pub fn build_from_tree(
        &self,
        tree: &VectorlessTree,
        node_id: NodeId,
        max_depth: usize,
    ) -> String {
        let mut sections = Vec::new();
        self.collect_sections(tree, node_id, 0, max_depth, &mut sections);
        sections.join(&self.separator)
    }

    fn collect_sections(
        &self,
        tree: &VectorlessTree,
        node_id: NodeId,
        current_depth: usize,
        max_depth: usize,
        sections: &mut Vec<String>,
    ) {
        if current_depth > max_depth {
            return;
        }

        if let Some(node) = tree.get(node_id) {
            let mut section = String::new();

            if self.include_titles {
                let indent = "  ".repeat(current_depth);
                section.push_str(&format!("{}# {}\n", indent, node.title));
            }

            if self.include_summaries && !node.summary.is_empty() {
                section.push_str(&format!("Summary: {}\n", node.summary));
            }

            if self.include_content && !node.content.is_empty() {
                section.push_str(&format!("\n{}\n", node.content));
            }

            if !section.is_empty() {
                sections.push(section);
            }

            // Recurse into children
            for child_id in tree.children(node_id) {
                self.collect_sections(tree, child_id, current_depth + 1, max_depth, sections);
            }
        }
    }

    fn format_section(&self, result: &RetrievalResult) -> String {
        let mut section = String::new();

        if self.include_titles {
            section.push_str(&format!("## {}\n", result.title));
        }

        if self.include_summaries {
            if let Some(summary) = &result.summary {
                section.push_str(&format!("Summary: {}\n", summary));
            }
        }

        if self.include_content {
            if let Some(content) = &result.content {
                section.push_str(&format!("\n{}\n", content));
            }
        }

        section
    }
}

/// Format retrieval results for LLM consumption.
pub fn format_for_llm(results: &[RetrievalResult], max_tokens: usize) -> String {
    ContextBuilder::new()
        .with_max_tokens(max_tokens)
        .build(results)
}

/// Format a document tree for LLM consumption.
pub fn format_tree_for_llm(
    tree: &VectorlessTree,
    max_depth: usize,
    max_tokens: usize,
) -> String {
    ContextBuilder::new()
        .with_max_tokens(max_tokens)
        .build_from_tree(tree, tree.root(), max_depth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_builder() {
        let results = vec![
            RetrievalResult::new("Section 1")
                .with_content("Content 1"),
            RetrievalResult::new("Section 2")
                .with_content("Content 2"),
        ];

        let context = ContextBuilder::new()
            .with_max_tokens(1000)
            .build(&results);

        assert!(context.contains("Section 1"));
        assert!(context.contains("Content 1"));
    }
}
