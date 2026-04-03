// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Adaptive retriever - main entry point for retrieval operations.
//!
//! Automatically selects the best strategy based on query complexity
//! and provides incremental retrieval with sufficiency checking.
//!
//! # Architecture
//!
//! The retriever uses a strategy pattern where each strategy implements
//! `RetrievalStrategy` trait. This decouples the retrieval logic from
//! the underlying LLM/embedding implementations.
//!
//! ```text
//! AdaptiveRetriever
//! ├── KeywordStrategy (always available, no deps)
//! ├── SemanticStrategy (optional, requires embeddings)
//! └── LlmStrategy (optional, requires LLM client)
//! ```

use async_trait::async_trait;
use std::sync::Arc;

use super::cache::PathCache;
use super::complexity::ComplexityDetector;
use super::retriever::{CostEstimate, Retriever, RetrieverError, RetrieverResult, RetrievalContext};
use super::search::SearchResult;
use super::strategy::{KeywordStrategy, LlmStrategy, RetrievalStrategy};
use super::sufficiency::{SufficiencyChecker, SufficiencyLevel, ThresholdChecker};
use super::types::{
    QueryComplexity, RetrieveOptions, RetrieveResponse, RetrievalResult,
    StrategyPreference,
};
use crate::config::RetrievalConfig as AppRetrievalConfig;
use crate::domain::{NodeId, DocumentTree, TocView};
use crate::llm::LlmClient;

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
    /// Minimum score threshold.
    pub min_score: f32,
    /// Top-k results.
    pub top_k: usize,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self::from_app_config(&AppRetrievalConfig::default())
    }
}

impl AdaptiveConfig {
    /// Create from application config.
    pub fn from_app_config(config: &AppRetrievalConfig) -> Self {
        Self {
            enable_cache: true,
            enable_sufficiency_check: true,
            beam_width: config.search.beam_width,
            max_iterations: config.search.max_iterations,
            min_score: config.search.min_score,
            top_k: config.search.top_k,
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

/// Adaptive retriever that combines multiple strategies.
///
/// This is the main entry point for retrieval operations. It:
/// 1. Detects query complexity
/// 2. Selects an appropriate strategy
/// 3. Executes tree search with strategy-guided scoring
/// 4. Checks sufficiency incrementally
/// 5. Returns aggregated results
///
/// # Strategies
///
/// - **Keyword**: Always available, uses TF-IDF/BM25 scoring
/// - **Semantic**: Optional, requires embedding model
/// - **LLM**: Optional, requires LLM client (uses `LlmStrategy`)
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::domain::retriever::AdaptiveRetriever;
/// use vectorless::llm::LlmClient;
///
/// // With LLM support
/// let retriever = AdaptiveRetriever::new()
///     .with_llm_strategy(LlmStrategy::with_defaults());
/// ```
pub struct AdaptiveRetriever {
    /// Configuration.
    config: AdaptiveConfig,
    /// Complexity detector.
    complexity_detector: ComplexityDetector,
    /// ToC view generator for LLM-guided navigation.
    toc_view: TocView,
    /// Keyword strategy (always available, no external deps).
    keyword_strategy: KeywordStrategy,
    /// Semantic strategy (optional, requires embedding model).
    semantic_strategy: Option<Arc<dyn RetrievalStrategy>>,
    /// LLM strategy (optional, uses `LlmStrategy`).
    llm_strategy: Option<Arc<LlmStrategy>>,
    /// Sufficiency checker.
    sufficiency_checker: Box<dyn SufficiencyChecker>,
    /// Path cache.
    cache: PathCache,
}

impl AdaptiveRetriever {
    /// Create a new adaptive retriever with default configuration.
    pub fn new() -> Self {
        Self {
            config: AdaptiveConfig::default(),
            complexity_detector: ComplexityDetector::new(),
            toc_view: TocView::new(),
            keyword_strategy: KeywordStrategy::new(),
            semantic_strategy: None,
            llm_strategy: None,
            sufficiency_checker: Box::new(ThresholdChecker::new()),
            cache: PathCache::new(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: AdaptiveConfig) -> Self {
        Self {
            config,
            complexity_detector: ComplexityDetector::new(),
            toc_view: TocView::new(),
            keyword_strategy: KeywordStrategy::new(),
            semantic_strategy: None,
            llm_strategy: None,
            sufficiency_checker: Box::new(ThresholdChecker::new()),
            cache: PathCache::new(),
        }
    }

    /// Set the LLM client for LLM-guided strategy.
    ///
    /// This creates an `LlmStrategy` internally using the provided client.
    pub fn with_llm_client(self, client: LlmClient) -> Self {
        self.with_llm_strategy(LlmStrategy::new(client))
    }

    /// Set a custom LLM strategy.
    ///
    /// Use this for fine-grained control over the LLM strategy,
    /// such as custom prompts or evaluation logic.
    pub fn with_llm_strategy(mut self, strategy: LlmStrategy) -> Self {
        self.llm_strategy = Some(Arc::new(strategy));
        self
    }

    /// Set the semantic strategy for semantic search.
    pub fn with_semantic_strategy(mut self, strategy: Box<dyn RetrievalStrategy>) -> Self {
        self.semantic_strategy = Some(Arc::from(strategy));
        self
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

    /// Execute tree search using the selected strategy.
    async fn execute_search(
        &self,
        tree: &DocumentTree,
        query: &str,
        options: &RetrieveOptions,
        context: &RetrievalContext,
    ) -> RetrieverResult<SearchResult> {
        let complexity = self.complexity_detector.detect(query);
        let strategy = self.select_strategy(complexity, options.strategy);

        tracing::info!(
            "Query complexity: {:?}, selected strategy: {}",
            complexity,
            strategy.name()
        );

        match strategy {
            SelectedStrategy::Keyword => {
                self.execute_keyword_search(tree, query, options, context).await
            }
            SelectedStrategy::Semantic => {
                self.execute_semantic_search(tree, query, options, context).await
            }
            SelectedStrategy::Llm => {
                self.execute_llm_search(tree, options, context).await
            }
        }
    }

    /// Execute keyword-based search (no LLM calls).
    async fn execute_keyword_search(
        &self,
        tree: &DocumentTree,
        query: &str,
        options: &RetrieveOptions,
        context: &RetrievalContext,
    ) -> RetrieverResult<SearchResult> {
        let mut results = SearchResult::default();
        let root = tree.root();

        // Use greedy search with keyword scoring
        let mut current = root;
        let mut path_score = 0.0f32;
        let mut visited = std::collections::HashSet::new();

        for _iteration in 0..options.max_iterations {
            results.iterations += 1;

            // Get children
            let children = tree.children(current);
            if children.is_empty() {
                // Leaf node - add as result
                if let Some(node) = tree.get(current) {
                    let score = self.keyword_strategy
                        .evaluate_node(tree, current, context)
                        .await
                        .score;

                    if score >= options.min_score {
                        results.paths.push(super::types::SearchPath::from_node(current, score));
                        results.trace.push(super::types::NavigationStep {
                            node_id: format!("{:?}", current),
                            title: node.title.clone(),
                            score,
                            decision: super::types::NavigationDecision::ThisIsTheAnswer,
                            depth: node.depth,
                        });
                    }
                }
                break;
            }

            // Score all children and pick best
            let mut best_child = None;
            let mut best_score = 0.0f32;

            for &child_id in &children {
                if visited.contains(&child_id) {
                    continue;
                }
                visited.insert(child_id);

                let eval = self.keyword_strategy
                    .evaluate_node(tree, child_id, context)
                    .await;

                results.nodes_visited += 1;
                results.trace.push(super::types::NavigationStep {
                    node_id: format!("{:?}", child_id),
                    title: tree.get(child_id).map(|n| n.title.clone()).unwrap_or_default(),
                    score: eval.score,
                    decision: eval.decision.clone().into(),
                    depth: tree.get(child_id).map(|n| n.depth).unwrap_or(0),
                });

                if eval.score > best_score {
                    best_score = eval.score;
                    best_child = Some(child_id);
                }
            }

            if let Some(child) = best_child {
                path_score += best_score;
                current = child;

                if results.paths.len() >= options.top_k {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(results)
    }

    /// Execute LLM-guided search using the LLM strategy.
    ///
    /// The query is taken from `context.query`.
    async fn execute_llm_search(
        &self,
        tree: &DocumentTree,
        options: &RetrieveOptions,
        context: &RetrievalContext,
    ) -> RetrieverResult<SearchResult> {
        let llm_strategy = self.llm_strategy.as_ref()
            .ok_or_else(|| RetrieverError::ConfigError(
                "LLM strategy not configured. Use with_llm_client() to set one.".to_string()
            ))?;

        let mut results = SearchResult::default();
        let root = tree.root();

        // LLM-guided beam search
        let beam_width = options.beam_width;
        let mut current_beam: Vec<(NodeId, f32)> = vec![(root, 1.0)];
        let mut visited = std::collections::HashSet::new();

        for _iteration in 0..options.max_iterations {
            results.iterations += 1;

            if current_beam.is_empty() {
                break;
            }

            let mut next_beam = Vec::new();

            for (node_id, path_score) in current_beam {
                if visited.contains(&node_id) {
                    continue;
                }
                visited.insert(node_id);

                let children = tree.children(node_id);

                // Leaf node
                if children.is_empty() {
                    if let Some(node) = tree.get(node_id) {
                        // Use LLM strategy to evaluate this leaf
                        let eval = llm_strategy
                            .evaluate_node(tree, node_id, context)
                            .await;

                        results.nodes_visited += 1;

                        if eval.score >= options.min_score {
                            results.paths.push(super::types::SearchPath::from_node(node_id, path_score * eval.score));
                            results.trace.push(super::types::NavigationStep {
                                node_id: format!("{:?}", node_id),
                                title: node.title.clone(),
                                score: eval.score,
                                decision: super::types::NavigationDecision::ThisIsTheAnswer,
                                depth: node.depth,
                            });
                        }
                    }
                    continue;
                }

                // Score children with LLM strategy
                for &child_id in &children {
                    let eval = llm_strategy
                        .evaluate_node(tree, child_id, context)
                        .await;

                    results.nodes_visited += 1;

                    results.trace.push(super::types::NavigationStep {
                        node_id: format!("{:?}", child_id),
                        title: tree.get(child_id).map(|n| n.title.clone()).unwrap_or_default(),
                        score: eval.score,
                        decision: eval.decision.clone().into(),
                        depth: tree.get(child_id).map(|n| n.depth).unwrap_or(0),
                    });

                    let child_score = path_score * eval.score;
                    next_beam.push((child_id, child_score));
                }

                // Limit beam width
                next_beam.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                next_beam.truncate(beam_width);
            }

            current_beam = next_beam;

            // Check if we have enough results
            if results.paths.len() >= options.top_k {
                break;
            }
        }

        Ok(results)
    }

    /// Convert search paths to retrieval results.
    fn paths_to_results(
        &self,
        tree: &DocumentTree,
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
                        result = result.with_content(&node.content)
                    }
                    if options.include_summaries && !node.summary.is_empty() {
                        result = result.with_summary(&node.summary)
                    }
                    if let (Some(start), Some(end)) = (node.start_page, node.end_page) {
                        result = result.with_page_range(start, end)
                    }
                    if let Some(ref node_id) = node.node_id {
                        result = result.with_node_id(node_id)
                    }

                    results.push(result)
                }
            }
        }

        Ok(results)
    }

    /// Execute semantic-based search (requires embedding model).
    async fn execute_semantic_search(
        &self,
        tree: &DocumentTree,
        query: &str,
        options: &RetrieveOptions,
        context: &RetrievalContext,
    ) -> RetrieverResult<SearchResult> {
        // Check if semantic strategy is available
        let semantic_strategy = self.semantic_strategy.as_ref()
            .ok_or_else(|| RetrieverError::ConfigError(
                "Semantic strategy not configured. Use with_semantic_strategy() to set one.".to_string()
            ))?;

        let mut results = SearchResult::default();
        let root = tree.root();

        tracing::info!("Starting semantic search for query: {}", query);

        // Use beam search with semantic scoring
        let beam_width = options.beam_width;
        let mut current_beam: Vec<(NodeId, f32)> = vec![(root, 1.0)];
        let mut visited = std::collections::HashSet::new();

        for _iteration in 0..options.max_iterations {
            results.iterations += 1;

            if current_beam.is_empty() {
                break;
            }

            let mut next_beam = Vec::new();

            for (node_id, path_score) in current_beam {
                if visited.contains(&node_id) {
                    continue;
                }
                visited.insert(node_id);

                let children = tree.children(node_id);

                // Leaf node
                if children.is_empty() {
                    if let Some(node) = tree.get(node_id) {
                        let eval = semantic_strategy
                            .evaluate_node(tree, node_id, context)
                            .await;

                        results.nodes_visited += 1;

                        if eval.score >= options.min_score {
                            results.paths.push(super::types::SearchPath::from_node(node_id, path_score * eval.score));
                            results.trace.push(super::types::NavigationStep {
                                node_id: format!("{:?}", node_id),
                                title: node.title.clone(),
                                score: eval.score,
                                decision: super::types::NavigationDecision::ThisIsTheAnswer,
                                depth: node.depth,
                            });
                        }
                    }
                    continue;
                }

                // Score children with semantic strategy
                for &child_id in &children {
                    if visited.contains(&child_id) {
                        continue;
                    }

                    let eval = semantic_strategy
                        .evaluate_node(tree, child_id, context)
                        .await;

                    results.nodes_visited += 1;

                    results.trace.push(super::types::NavigationStep {
                        node_id: format!("{:?}", child_id),
                        title: tree.get(child_id).map(|n| n.title.clone()).unwrap_or_default(),
                        score: eval.score,
                        decision: eval.decision.clone().into(),
                        depth: tree.get(child_id).map(|n| n.depth).unwrap_or(0),
                    });

                    let child_score = path_score * eval.score;
                    next_beam.push((child_id, child_score));
                }
            }

            // Limit beam width
            next_beam.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            next_beam.truncate(beam_width);

            current_beam = next_beam;

            // Check if we have enough results
            if results.paths.len() >= options.top_k {
                break;
            }
        }

        tracing::info!(
            "Semantic search completed: {} paths found, {} nodes visited, {} iterations",
            results.paths.len(),
            results.nodes_visited,
            results.iterations
        );

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

    /// Generate a ToC view for LLM context.
    ///
    /// This creates a hierarchical view of the document structure
    /// that can be used for LLM-guided navigation.
    pub fn generate_toc_view(&self, tree: &DocumentTree) -> String {
        let toc = self.toc_view.generate(tree);
        self.toc_view.format_markdown(&toc)
    }

    /// Generate a ToC view from a specific node.
    pub fn generate_toc_from(&self, tree: &DocumentTree, node_id: NodeId) -> String {
        let toc = self.toc_view.generate_from(tree, node_id);
        self.toc_view.format_markdown(&toc)
    }

    /// Generate a flat ToC list for quick scanning.
    pub fn generate_flat_toc(&self, tree: &DocumentTree) -> Vec<crate::domain::TocEntry> {
        self.toc_view.generate_flat(tree)
    }

    /// Generate a filtered ToC based on criteria.
    pub fn generate_filtered_toc<F>(
        &self,
        tree: &DocumentTree,
        filter: F,
    ) -> Vec<crate::domain::TocNode>
    where
        F: Fn(&crate::domain::TreeNode) -> bool,
    {
        self.toc_view.generate_filtered(tree, filter)
    }
}

impl Default for AdaptiveRetriever {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for AdaptiveRetriever {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdaptiveRetriever")
            .field("config", &self.config)
            .field("enable_cache", &self.config.enable_cache)
            .field("enable_sufficiency_check", &self.config.enable_sufficiency_check)
            .finish()
    }
}

#[async_trait]
impl Retriever for AdaptiveRetriever {
    async fn retrieve(
        &self,
        tree: &DocumentTree,
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
        let strategy = self.select_strategy(complexity, options.strategy);
        // Check cache first
        if self.config.enable_cache {
            if let Some(cached_paths) = self.cache.get_paths(query) {
                let results = self.paths_to_results(tree, cached_paths, options)?;
                return Ok(RetrieveResponse {
                    results,
                    content: String::new(),
                    confidence: 0.0,
                    is_sufficient: true,
                    strategy_used: strategy.name().to_string(),
                    complexity,
                    trace: Vec::new(),
                    tokens_used: 0,
                });
            }
        }
        // Execute search
        let search_result = self.execute_search(tree, query, options, &context).await?;
        // Convert paths to results
        let paths_clone = search_result.paths.clone();
        let mut results = self.paths_to_results(tree, paths_clone, options)?;
        if results.is_empty() {
            return Err(RetrieverError::NoResults);
        }
        // Cache results
        if self.config.enable_cache {
            self.cache.store_paths(query, search_result.paths);
        }
        // Incremental retrieval with sufficiency checking
        let mut aggregated_content = String::new();
        let mut tokens_used = 0;
        if options.sufficiency_check {
            for result in &results {
                let content_part = match (&result.content, &result.summary) {
                    (Some(content), _) => content.clone(),
                    (_, Some(summary)) => summary.clone(),
                    (None, None) => continue,
                };
                aggregated_content.push_str(&content_part);
                aggregated_content.push_str("\n\n");
                tokens_used = self.estimate_tokens(&aggregated_content);
                // Check sufficiency
                let sufficiency = self.sufficiency_checker
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
        Ok(RetrieveResponse {
            results,
            content: aggregated_content,
            confidence,
            is_sufficient,
            strategy_used: strategy.name().to_string(),
            complexity,
            trace: search_result.trace,
            tokens_used,
        })
    }

    fn name(&self) -> &str {
        "adaptive"
    }

    fn estimate_cost(&self, tree: &DocumentTree, options: &RetrieveOptions) -> CostEstimate {
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
                tokens: node_count * 100, // embedding tokens
            },
            SelectedStrategy::Llm => CostEstimate {
                llm_calls: options.max_iterations.min(node_count),
                tokens: options.max_iterations * 500,
            },
        }
    }
}
