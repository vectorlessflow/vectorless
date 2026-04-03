// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Analyze Stage - Query analysis and information extraction.
//!
//! This stage analyzes the query to determine:
//! - Query complexity (Simple/Medium/Complex)
//! - Keywords for matching
//! - Target sections based on ToC matching

use async_trait::async_trait;
use tracing::info;

use crate::domain::{DocumentTree, TocView};
use crate::retrieval::complexity::ComplexityDetector;
use crate::retrieval::pipeline::{
    FailurePolicy, PipelineContext, RetrievalStage, StageOutcome,
};
// QueryComplexity is used in context

/// Analyze Stage - analyzes queries for retrieval planning.
///
/// This stage:
/// 1. Detects query complexity (Simple/Medium/Complex)
/// 2. Extracts keywords for matching
/// 3. Matches target sections from ToC
///
/// # Example
///
/// ```rust,ignore
/// let stage = AnalyzeStage::new()
///     .with_toc_matching(true);
/// ```
pub struct AnalyzeStage {
    complexity_detector: ComplexityDetector,
    toc_view: TocView,
    enable_toc_matching: bool,
}

impl Default for AnalyzeStage {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyzeStage {
    /// Create a new analyze stage.
    pub fn new() -> Self {
        Self {
            complexity_detector: ComplexityDetector::new(),
            toc_view: TocView::new(),
            enable_toc_matching: true,
        }
    }

    /// Enable or disable ToC section matching.
    pub fn with_toc_matching(mut self, enable: bool) -> Self {
        self.enable_toc_matching = enable;
        self
    }

    /// Extract keywords from a query.
    fn extract_keywords(&self, query: &str) -> Vec<String> {
        // Simple keyword extraction:
        // 1. Lowercase
        // 2. Split on whitespace
        // 3. Remove common stop words
        // 4. Remove short words (< 2 chars)
        // 5. Remove punctuation

        let stop_words = [
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
            "have", "has", "had", "do", "does", "did", "will", "would", "could",
            "should", "may", "might", "must", "shall", "can", "need", "dare",
            "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by",
            "from", "as", "into", "through", "during", "before", "after",
            "above", "below", "between", "under", "again", "further", "then",
            "once", "here", "there", "when", "where", "why", "how", "all",
            "each", "few", "more", "most", "other", "some", "such", "no", "nor",
            "not", "only", "own", "same", "so", "than", "too", "very", "just",
            "and", "but", "if", "or", "because", "until", "while", "although",
            "though", "what", "which", "who", "whom", "this", "that", "these",
            "those", "am", "it", "its", "itself", "he", "him", "his", "she",
            "her", "hers", "they", "them", "their", "we", "us", "our", "you",
            "your", "i", "me", "my",
        ];

        query
            .to_lowercase()
            .split_whitespace()
            .filter(|word| {
                let word = word.trim_matches(|c: char| !c.is_alphanumeric());
                word.len() >= 2 && !stop_words.contains(&word)
            })
            .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|word| !word.is_empty())
            .collect()
    }

    /// Match target sections from ToC.
    fn match_toc_sections(&self, query: &str, tree: &DocumentTree) -> Vec<String> {
        if !self.enable_toc_matching {
            return Vec::new();
        }

        let toc = self.toc_view.generate_from(tree, tree.root());
        let query_lower = query.to_lowercase();

        // Find sections that match query keywords
        let mut matches: Vec<(String, f32)> = Vec::new();

        fn collect_sections(
            nodes: &[crate::domain::TocNode],
            query_lower: &str,
            matches: &mut Vec<(String, f32)>,
        ) {
            for node in nodes {
                let title_lower = node.title.to_lowercase();

                // Calculate match score
                let mut score = 0.0f32;

                // Exact title match
                if title_lower.contains(query_lower) {
                    score = 1.0;
                } else {
                    // Partial word matches
                    for word in query_lower.split_whitespace() {
                        if title_lower.contains(word) {
                            score += 0.3;
                        }
                    }
                }

                if score > 0.0 {
                    matches.push((node.title.clone(), score));
                }

                // Recurse into children
                collect_sections(&node.children, query_lower, matches);
            }
        }

        collect_sections(&toc.children, &query_lower, &mut matches);

        // Sort by score and return top sections
        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        matches.into_iter().take(5).map(|(title, _)| title).collect()
    }
}

#[async_trait]
impl RetrievalStage for AnalyzeStage {
    fn name(&self) -> &str {
        "analyze"
    }

    fn priority(&self) -> i32 {
        10 // First stage
    }

    fn failure_policy(&self) -> FailurePolicy {
        FailurePolicy::fail() // Must succeed
    }

    async fn execute(&self, ctx: &mut PipelineContext) -> crate::domain::Result<StageOutcome> {
        info!("Analyzing query: '{}'", ctx.query);

        // 1. Detect complexity
        ctx.complexity = Some(self.complexity_detector.detect(&ctx.query));
        info!(
            "Query complexity: {:?}",
            ctx.complexity
        );

        // 2. Extract keywords
        ctx.keywords = self.extract_keywords(&ctx.query);
        info!("Extracted keywords: {:?}", ctx.keywords);

        // 3. Match target sections
        ctx.target_sections = self.match_toc_sections(&ctx.query, &ctx.tree);
        if !ctx.target_sections.is_empty() {
            info!("Target sections: {:?}", ctx.target_sections);
        }

        // 4. Update metrics
        ctx.metrics.llm_calls += 0; // No LLM calls in this stage

        Ok(StageOutcome::cont())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_keywords() {
        let stage = AnalyzeStage::new();

        let keywords = stage.extract_keywords("What is the architecture of the system?");
        assert!(!keywords.contains(&"the".to_string()));
        assert!(keywords.contains(&"architecture".to_string()));
        assert!(keywords.contains(&"system".to_string()));
    }

    #[test]
    fn test_extract_keywords_empty() {
        let stage = AnalyzeStage::new();
        let keywords = stage.extract_keywords("");
        assert!(keywords.is_empty());
    }

    #[test]
    fn test_extract_keywords_stopwords() {
        let stage = AnalyzeStage::new();
        let keywords = stage.extract_keywords("the a an is are was were");
        assert!(keywords.is_empty());
    }
}
