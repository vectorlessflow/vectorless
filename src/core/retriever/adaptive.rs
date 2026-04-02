// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Adaptive retriever - main entry point for retrieval operations.
//!
//! Automatically selects the best strategy based on query complexity
//! and provides incremental retrieval with sufficiency checking.

use async_trait::async_trait;

use super::cache::PathCache;
use super::complexity::ComplexityDetector;
use super::retriever::{CostEstimate, Retriever, RetrieverError, RetrieverResult, RetrievalContext};
use super::search::{BeamSearch, GreedySearch, SearchConfig, SearchTree};
use super::strategy::{KeywordStrategy, RetrievalStrategy};
use super::sufficiency::{SufficiencyChecker, ThresholdChecker};
use super::types::{
    QueryComplexity, RetrieveOptions, RetrieveResponse, RetrievalResult, StrategyPreference,
    SufficiencyLevel,
};
use crate::core::{NodeId, VectorlessTree};

/// Configuration for the adaptive retriever.
#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    /// Whether to enable caching.
    pub enable_cache: bool,
    /// Whether to enable sufficiency checking.
    pub enable_sufficiency_check: bool,
    /// Default beam width.
    pub beam_width: usize,
    /// Maximum search iterations.
    pub max_iterations: usize,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            enable_cache: true,
            enable_sufficiency_check: true,
            beam_width: 3,
            max_iterations: 10,
        }
    }
}

/// Adaptive retriever that combines multiple strategies.
///
/// This is the main entry point for retrieval operations. It:
/// 1. Detects query complexity
/// 2. Selects an appropriate strategy
/// 3. Executes multi-path search
/// 4. Checks sufficiency incrementally
/// 5. Returns aggregated results
pub struct AdaptiveRetriever {
    /// Configuration.
    config: AdaptiveConfig,
    /// Complexity detector.
    complexity_detector: ComplexityDetector,
    /// Keyword strategy (always available, no external deps).
    keyword_strategy: KeywordStrategy,
    /// Sufficiency checker.
    sufficiency_checker: Box<dyn SufficiencyChecker>,
    /// Path cache.
    cache: PathCache,
}

impl AdaptiveRetriever {
    /// Create a new adaptive retriever.
    pub fn new() -> Self {
        Self {
            config: AdaptiveConfig::default(),
            complexity_detector: ComplexityDetector::new(),
            keyword_strategy: KeywordStrategy::new(),
            sufficiency_checker: Box::new(ThresholdChecker::new()),
            cache: PathCache::new(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: AdaptiveConfig) -> Self {
        Self {
            cache: PathCache::new(),
            config,
            complexity_detector: ComplexityDetector::new(),
            keyword_strategy: KeywordStrategy::new(),
            sufficiency_checker: Box::new(ThresholdChecker::new()),
        }
    }

    /// Set a custom sufficiency checker.
    pub fn with_sufficiency_checker(mut self, checker: Box<dyn SufficiencyChecker>) -> Self {
        self.sufficiency_checker = checker;
        self
    }

    /// Select strategy based on query complexity and preference.
    fn select_strategy(
        &self,
        complexity: QueryComplexity,
        preference: StrategyPreference,
    ) -> SelectedStrategy {
        match preference {
            StrategyPreference::ForceKeyword => SelectedStrategy::Keyword,
            StrategyPreference::ForceSemantic => SelectedStrategy::Semantic,
            StrategyPreference::ForceLlm => SelectedStrategy::Llm,
            StrategyPreference::Auto => match complexity {
                QueryComplexity::Simple => SelectedStrategy::Keyword,
                QueryComplexity::Medium => SelectedStrategy::Semantic,
                QueryComplexity::Complex => SelectedStrategy::Llm,
            },
        }
    }

    /// Execute retrieval using the selected strategy.
    async fn execute_retrieval(
        &self,
        tree: &VectorlessTree,
        query: &str,
        options: &RetrieveOptions,
        context: &RetrievalContext,
    ) -> RetrieverResult<Vec<RetrievalResult>> {
        let complexity = self.complexity_detector.detect(query);
        let strategy = self.select_strategy(complexity, options.strategy);

        // Check cache first
        if self.config.enable_cache {
            if let Some(cached_paths) = self.cache.get_paths(query) {
                return self.paths_to_results(tree, cached_paths, options);
            }
        }

        // Configure search
        let search_config = SearchConfig {
            top_k: options.top_k,
            beam_width: options.beam_width,
            max_iterations: options.max_iterations,
            min_score: options.min_score,
            leaf_only: true,
        };

        // Execute search based on strategy
        let search_result = match strategy {
            SelectedStrategy::Keyword => {
                let search = GreedySearch::new();
                search.search(tree, context, &search_config).await
            }
            SelectedStrategy::Semantic | SelectedStrategy::Llm => {
                // Use beam search for more complex strategies
                let search = BeamSearch::with_width(options.beam_width);
                search.search(tree, context, &search_config).await
            }
        };

        // Convert paths to results
        let results = self.paths_to_results(tree, search_result.paths, options)?;

        // Cache results
        if self.config.enable_cache {
            self.cache.store_paths(query, search_result.paths.clone());
        }

        Ok(results)
    }

    /// Convert search paths to retrieval results.
    fn paths_to_results(
        &self,
        tree: &VectorlessTree,
        paths: Vec<super::types::SearchPath>,
        options: &RetrieveOptions,
    ) -> RetrieverResult<Vec<RetrievalResult>> {
        let mut results = Vec::new();

        for path in paths {
            if let Some(leaf_id) = path.leaf {
                if let Some(node) = tree.get(leaf_id) {
                    let mut result = RetrievalResult::new(&node.title)
                        .with_score(path.score)
                        .with_depth(node.depth);

                    if options.include_content {
                        result = result.with_content(&node.content);
                    }
                    if options.include_summaries && !node.summary.is_empty() {
                        result = result.with_summary(&node.summary);
                    }
                    if let (Some(start), Some(end)) = (node.start_page, node.end_page) {
                        result = result.with_page_range(start, end);
                    }
                    if let Some(ref node_id) = node.node_id {
                        result = result.with_node_id(node_id);
                    }

                    results.push(result);
                }
            }
        }

        Ok(results)
    }

    /// Aggregate content from results.
    fn aggregate_content(&self, results: &[RetrievalResult]) -> String {
        results
            .iter()
            .filter_map(|r| {
                if let Some(content) = &r.content {
                    Some(format!("=== {} ===\n{}", r.title, content))
                } else if let Some(summary) = &r.summary {
                    Some(format!("=== {} (Summary) ===\n{}", r.title, summary))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Estimate token count for content.
    fn estimate_tokens(&self, content: &str) -> usize {
        content.len() / 4
    }
}

impl Default for AdaptiveRetriever {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Retriever for AdaptiveRetriever {
    async fn retrieve(
        &self,
        tree: &VectorlessTree,
        query: &str,
        options: &RetrieveOptions,
    ) -> RetrieverResult<RetrieveResponse> {
        // Validate tree
        if tree.node_count() == 0 {
            return Err(RetrieverError::InvalidTree("Tree is empty".to_string()));
        }

        // Create retrieval context
        let context = RetrievalContext::new(query, options.max_tokens, options.sufficiency_check);

        // Detect complexity
        let complexity = self.complexity_detector.detect(query);

        // Execute retrieval
        let mut results = self.execute_retrieval(tree, query, options, &context).await?;

        if results.is_empty() {
            return Err(RetrieverError::NoResults);
        }

        // Incremental retrieval with sufficiency checking
        let mut aggregated_content = String::new();
        let mut tokens_used = 0;

        if options.sufficiency_check {
            for result in &results {
                let content_part = if let Some(content) = &result.content {
                    content.clone()
                } else if let Some(summary) = &result.summary {
                    summary.clone()
                } else {
                    continue;
                };

                aggregated_content.push_str(&content_part);
                aggregated_content.push_str("\n\n");

                tokens_used = self.estimate_tokens(&aggregated_content);

                // Check sufficiency
                let sufficiency = self
                    .sufficiency_checker
                    .check(query, &aggregated_content, tokens_used);

                if matches!(sufficiency, SufficiencyLevel::Sufficient) {
                    break;
                }
            }
        } else {
            aggregated_content = self.aggregate_content(&results);
            tokens_used = self.estimate_tokens(&aggregated_content);
        }

        // Calculate confidence score
        let confidence = if results.is_empty() {
            0.0
        } else {
            results.iter().map(|r| r.score).sum::<f32>() / results.len() as f32
        };

        // Determine if sufficient
        let is_sufficient = matches!(
            self.sufficiency_checker
                .check(query, &aggregated_content, tokens_used),
            SufficiencyLevel::Sufficient | SufficiencyLevel::PartialSufficient
        );

        // Get strategy name
        let strategy_used = self.select_strategy(complexity, options.strategy).name();

        Ok(RetrieveResponse {
            results,
            content: aggregated_content,
            confidence,
            is_sufficient,
            strategy_used: strategy_used.to_string(),
            complexity,
            trace: Vec::new(), // TODO: collect trace from search
            tokens_used,
        })
    }

    fn name(&self) -> &str {
        "adaptive"
    }

    fn estimate_cost(&self, tree: &VectorlessTree, options: &RetrieveOptions) -> CostEstimate {
        let node_count = tree.node_count();

        // Estimate based on strategy
        let complexity = self.complexity_detector.detect("sample query");
        let strategy = self.select_strategy(complexity, options.strategy);

        match strategy {
            SelectedStrategy::Keyword => CostEstimate {
                llm_calls: 0,
                tokens: 0,
            },
            SelectedStrategy::Semantic => CostEstimate {
                llm_calls: 0,
                tokens: node_count * 100,
            },
            SelectedStrategy::Llm => CostEstimate {
                llm_calls: options.max_iterations,
                tokens: options.max_iterations * 500,
            },
        }
    }
}

/// Internal enum for strategy selection.
#[derive(Debug, Clone, Copy)]
enum SelectedStrategy {
    Keyword,
    Semantic,
    Llm,
}

impl SelectedStrategy {
    fn name(&self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::Semantic => "semantic",
            Self::Llm => "llm",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_selection() {
        let retriever = AdaptiveRetriever::new();

        assert!(matches!(
            retriever.select_strategy(QueryComplexity::Simple, StrategyPreference::Auto),
            SelectedStrategy::Keyword
        ));

        assert!(matches!(
            retriever.select_strategy(QueryComplexity::Medium, StrategyPreference::Auto),
            SelectedStrategy::Semantic
        ));

        assert!(matches!(
            retriever.select_strategy(QueryComplexity::Complex, StrategyPreference::Auto),
            SelectedStrategy::Llm
        ));

        assert!(matches!(
            retriever.select_strategy(QueryComplexity::Simple, StrategyPreference::ForceLlm),
            SelectedStrategy::Llm
        ));
    }
}
