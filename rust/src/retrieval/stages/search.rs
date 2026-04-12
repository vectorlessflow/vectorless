// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Search Stage - Execute tree search with Pilot integration.
//!
//! This stage executes the selected search algorithm using
//! hierarchical ToC-based location followed by tree traversal.
//! When a Pilot is provided, it can provide semantic guidance
//! at key decision points.

use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::document::DocumentTree;
use crate::document::ReasoningIndex;
use crate::llm::LlmClient;
use crate::retrieval::RetrievalContext;
use crate::retrieval::cache::CachedCandidate;
use crate::retrieval::pilot::Pilot;
use crate::retrieval::pipeline::{
    CandidateNode, FailurePolicy, PipelineContext, RetrievalStage, SearchAlgorithm, StageOutcome,
};
use crate::retrieval::search::extract_keywords;
use crate::retrieval::search::{
    BeamSearch, GreedySearch, SearchConfig as SearchAlgConfig, SearchCue, SearchTree, ToCNavigator,
};
use crate::retrieval::strategy::{
    HybridConfig, HybridStrategy, KeywordStrategy, LlmStrategy, RetrievalStrategy,
};
use crate::retrieval::types::{
    NavigationDecision, ReasoningCandidate, ReasoningStep, StageName, StrategyPreference,
};

/// Search Stage - executes tree search with optional Pilot guidance.
///
/// This stage:
/// 1. Uses ToCNavigator to locate relevant subtrees (Phase Locate)
/// 2. Resolves queries (original or decomposed sub-queries)
/// 3. Runs search algorithms from located subtrees (Phase Traverse)
/// 4. Collects and deduplicates candidates (Phase Collect)
///
/// # Pilot Integration
///
/// When a Pilot is provided via [`with_pilot`], the search algorithm
/// can consult it at key decision points for semantic guidance.
/// Without a Pilot, the search uses pure algorithm scoring.
pub struct SearchStage {
    keyword_strategy: KeywordStrategy,
    llm_strategy: Option<Arc<LlmStrategy>>,
    hybrid_strategy: Option<Arc<dyn RetrievalStrategy>>,
    /// Pilot for navigation guidance (optional).
    pilot: Option<Arc<dyn Pilot>>,
    /// LLM client for ToC-based location (optional).
    llm_client: Option<LlmClient>,
    /// ToC navigator for hierarchical subtree location.
    toc_navigator: ToCNavigator,
}

impl Default for SearchStage {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchStage {
    /// Create a new search stage without Pilot.
    pub fn new() -> Self {
        Self {
            keyword_strategy: KeywordStrategy::new(),
            llm_strategy: None,
            hybrid_strategy: None,
            pilot: None,
            llm_client: None,
            toc_navigator: ToCNavigator::new(),
        }
    }

    /// Add LLM client for ToC-based search.
    pub fn with_llm_client(mut self, client: Option<LlmClient>) -> Self {
        if let Some(ref client) = client {
            self.toc_navigator = ToCNavigator::new().with_llm_client(client.clone());
        }
        self.llm_client = client;
        self
    }

    /// Add Pilot for semantic navigation guidance.
    pub fn with_pilot(mut self, pilot: Arc<dyn Pilot>) -> Self {
        self.pilot = Some(pilot);
        self
    }

    /// Add LLM strategy for complex queries.
    pub fn with_llm_strategy(mut self, strategy: LlmStrategy) -> Self {
        self.llm_strategy = Some(Arc::new(strategy));
        self
    }

    /// Add hybrid strategy (BM25 + LLM refinement).
    pub fn with_hybrid_strategy(mut self, strategy: Arc<dyn RetrievalStrategy>) -> Self {
        self.hybrid_strategy = Some(strategy);
        self
    }

    /// Configure hybrid strategy with custom config using the LLM strategy.
    pub fn with_hybrid_config(mut self, config: HybridConfig) -> Self {
        if let Some(ref llm) = self.llm_strategy {
            let llm_boxed: Box<dyn RetrievalStrategy> = Box::new((**llm).clone());
            self.hybrid_strategy =
                Some(Arc::new(HybridStrategy::new(llm_boxed).with_config(config)));
        }
        self
    }

    /// Check if Pilot is available and active.
    pub fn has_pilot(&self) -> bool {
        self.pilot.as_ref().map(|p| p.is_active()).unwrap_or(false)
    }

    /// Get the strategy to use based on context.
    fn get_strategy(&self, ctx: &PipelineContext) -> Arc<dyn RetrievalStrategy> {
        let preference = ctx.selected_strategy.unwrap_or(StrategyPreference::Auto);

        match preference {
            StrategyPreference::ForceKeyword => {
                info!("Using Keyword strategy");
                Arc::new(self.keyword_strategy.clone())
            }
            StrategyPreference::ForceLlm => {
                if let Some(ref strategy) = self.llm_strategy {
                    info!("Using LLM strategy");
                    strategy.clone()
                } else {
                    warn!("LLM strategy requested but not available, falling back to Keyword");
                    Arc::new(self.keyword_strategy.clone())
                }
            }
            StrategyPreference::ForceHybrid => {
                if let Some(ref strategy) = self.hybrid_strategy {
                    info!("Using Hybrid strategy");
                    strategy.clone()
                } else if let Some(ref llm) = self.llm_strategy {
                    info!("Using Hybrid strategy (auto-created from LLM)");
                    let llm_boxed: Box<dyn RetrievalStrategy> = Box::new((**llm).clone());
                    Arc::new(HybridStrategy::new(llm_boxed))
                } else {
                    warn!(
                        "Hybrid strategy requested but no LLM available, falling back to Keyword"
                    );
                    Arc::new(self.keyword_strategy.clone())
                }
            }
            StrategyPreference::ForceCrossDocument | StrategyPreference::ForcePageRange => {
                if let Some(ref strategy) = self.hybrid_strategy {
                    info!("Using Hybrid strategy as fallback for {:?})", preference);
                    strategy.clone()
                } else {
                    warn!(
                        "{:?} requires special configuration, falling back to Keyword",
                        preference
                    );
                    Arc::new(self.keyword_strategy.clone())
                }
            }
            StrategyPreference::Auto => Arc::new(self.keyword_strategy.clone()),
        }
    }

    /// Extract candidates from search paths.
    fn extract_candidates(
        &self,
        paths: &[crate::retrieval::types::SearchPath],
        tree: &DocumentTree,
    ) -> Vec<CandidateNode> {
        let mut candidates = Vec::new();

        for path in paths {
            if let Some(leaf_id) = path.leaf {
                if let Some(node) = tree.get(leaf_id) {
                    let depth = node.depth;
                    let is_leaf = tree.is_leaf(leaf_id);
                    candidates.push(CandidateNode::new(leaf_id, path.score, depth, is_leaf));
                }
            }
        }

        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates
    }

    /// Resolve the list of queries to search for.
    ///
    /// If decomposition produced multi-turn sub-queries, returns them in
    /// execution order. Otherwise returns the original query.
    fn resolve_queries(ctx: &PipelineContext) -> Vec<String> {
        if let Some(ref decomp) = ctx.decomposition {
            if decomp.was_decomposed && decomp.is_multi_turn() {
                return decomp
                    .execution_order()
                    .iter()
                    .map(|&i| decomp.sub_queries[i].text.clone())
                    .collect();
            }
        }
        vec![ctx.query.clone()]
    }

    /// Run search across all queries and cues, collecting and deduplicating results.
    async fn run_search(
        &self,
        ctx: &mut PipelineContext,
        queries: &[String],
        cues: &[SearchCue],
    ) -> (Vec<crate::retrieval::types::SearchPath>, Vec<CandidateNode>) {
        let algorithm = ctx.selected_algorithm.unwrap_or(SearchAlgorithm::Beam);
        let config = ctx.search_config.clone().unwrap_or_default();

        let search_config = SearchAlgConfig {
            top_k: config.beam_width * 2,
            beam_width: config.beam_width,
            max_iterations: config.max_iterations,
            min_score: config.min_score,
            leaf_only: false,
        };

        let pilot_ref: Option<&dyn Pilot> = self.pilot.as_deref();

        let mut all_paths = Vec::new();
        let mut total_pilot_interventions = 0u64;

        for query in queries {
            let legacy_ctx =
                RetrievalContext::new(query, ctx.options.max_tokens, ctx.options.sufficiency_check);

            for cue in cues {
                debug!(
                    "Searching: algorithm={:?}, query='{}', cue.root={:?}, cue.confidence={:.3}",
                    algorithm, query, cue.root, cue.confidence
                );

                let result = match algorithm {
                    SearchAlgorithm::Greedy => {
                        GreedySearch::new()
                            .search_from(
                                &ctx.tree,
                                &legacy_ctx,
                                &search_config,
                                pilot_ref,
                                cue.root,
                            )
                            .await
                    }
                    SearchAlgorithm::Beam => {
                        BeamSearch::new()
                            .search_from(
                                &ctx.tree,
                                &legacy_ctx,
                                &search_config,
                                pilot_ref,
                                cue.root,
                            )
                            .await
                    }
                    // MCTS is not truly implemented — falls back to Beam behavior.
                    SearchAlgorithm::Mcts => {
                        BeamSearch::new()
                            .search_from(
                                &ctx.tree,
                                &legacy_ctx,
                                &search_config,
                                pilot_ref,
                                cue.root,
                            )
                            .await
                    }
                };

                all_paths.extend(result.paths);
                total_pilot_interventions += result.pilot_interventions as u64;
            }
        }

        let mut all_candidates = self.extract_candidates(&all_paths, &ctx.tree);

        // Deduplicate by node_id, keeping the highest-scored entry
        all_candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all_candidates.dedup_by(|a, b| a.node_id == b.node_id);

        info!(
            "Search complete: {} paths, {} candidates (pilot interventions: {})",
            all_paths.len(),
            all_candidates.len(),
            total_pilot_interventions
        );

        (all_paths, all_candidates)
    }

    /// Check if a query is asking for a document summary/overview.
    fn is_summary_query(query: &str) -> bool {
        let lower = query.to_lowercase();

        // Direct keyword matches
        let patterns = [
            "summarize",
            "summary",
            "overview",
            "give me an overview",
            "describe this document",
            "main topics",
            "table of contents",
            "这篇文档讲了什么",
            "总结",
            "概述",
            "概要",
            "主要内容",
            "文档简介",
            "介绍一下",
        ];
        if patterns.iter().any(|p| lower.contains(p)) {
            return true;
        }

        // Phrase patterns — match with intervening words removed.
        // "what is this project about" → remove common filler words, check for "what is this about"
        let filler_words = ["project", "document", "file", "paper", "article", "text", "book", "the", "a", "an"];
        let cleaned: String = lower
            .split_whitespace()
            .filter(|w| !filler_words.contains(w))
            .collect::<Vec<_>>()
            .join(" ");

        let phrase_patterns = [
            "what is this about",
            "what is this document",
            "what is this about",
            "what does this mean",
            "tell me about this",
            "what is the main idea",
            "what are the key points",
            "what is the purpose",
        ];
        phrase_patterns.iter().any(|p| cleaned.contains(p))
    }

    /// Try to match the query against pre-computed reasoning index entries.
    ///
    /// Returns candidates if a high-confidence match is found, None otherwise.
    fn try_reasoning_shortcut(
        ridx: &ReasoningIndex,
        ctx: &PipelineContext,
    ) -> Option<Vec<CandidateNode>> {
        // Check 1: Summary shortcut — handle "overview" style queries
        if let Some(ref shortcut) = ridx.summary_shortcut() {
            if Self::is_summary_query(&ctx.query) {
                // For summary queries, return all top-level sections as candidates.
                // Don't include the root node itself — it has no direct content,
                // only descendant leaf content which is already covered by sections.
                let candidates: Vec<CandidateNode> = shortcut
                    .section_summaries
                    .iter()
                    .map(|section| {
                        CandidateNode::new(
                            section.node_id,
                            1.0,
                            section.depth,
                            ctx.tree.is_leaf(section.node_id),
                        )
                    })
                    .collect();

                if !candidates.is_empty() {
                    return Some(candidates);
                }

                // Fallback: if no sections, use root node
                return Some(vec![CandidateNode::new(
                    shortcut.root_node,
                    1.0,
                    0,
                    ctx.tree.is_leaf(shortcut.root_node),
                )]);
            }
        }

        // Check 2: Keyword → Topic path matching
        let keywords = extract_keywords(&ctx.query);
        if keywords.is_empty() {
            return None;
        }

        let mut scored_nodes: std::collections::HashMap<crate::document::NodeId, f32> =
            std::collections::HashMap::new();
        for keyword in &keywords {
            if let Some(entries) = ridx.topic_entries(keyword) {
                for entry in entries {
                    let score = scored_nodes.entry(entry.node_id).or_insert(0.0);
                    *score += entry.weight;
                }
            }
        }

        if scored_nodes.is_empty() {
            return None;
        }

        // Boost hot nodes by 20%
        for (node_id, score) in scored_nodes.iter_mut() {
            if ridx.is_hot(*node_id) {
                *score *= 1.2;
            }
        }

        // Convert to candidates, only return if best match is high-confidence
        let mut candidates: Vec<CandidateNode> = scored_nodes
            .into_iter()
            .filter_map(|(node_id, score)| {
                let depth = ctx.tree.get(node_id).map(|n| n.depth)?;
                Some(CandidateNode::new(
                    node_id,
                    score,
                    depth,
                    ctx.tree.is_leaf(node_id),
                ))
            })
            .collect();

        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Only return shortcut results if we have a high-confidence match
        let best_score = candidates.first().map(|c| c.score).unwrap_or(0.0);
        if best_score > 0.5 {
            Some(candidates)
        } else {
            None
        }
    }
}

#[async_trait]
impl RetrievalStage for SearchStage {
    fn name(&self) -> &str {
        "search"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["plan"]
    }

    fn priority(&self) -> i32 {
        30
    }

    fn failure_policy(&self) -> FailurePolicy {
        FailurePolicy::retry()
    }

    fn can_backtrack(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &mut PipelineContext) -> crate::error::Result<StageOutcome> {
        let start = std::time::Instant::now();

        let algorithm = ctx.selected_algorithm.unwrap_or(SearchAlgorithm::Beam);
        let config = ctx.search_config.clone().unwrap_or_default();

        // Budget check: skip search iteration if exhausted
        let budget_status = ctx.budget_controller.status();
        if budget_status.should_stop() && ctx.search_iterations > 0 {
            info!(
                "Budget exhausted ({}/{}), skipping search iteration",
                ctx.budget_controller.consumed(),
                ctx.budget_controller.total_budget(),
            );
            ctx.record_reasoning(
                StageName::Search,
                format!(
                    "Budget exhausted ({}/{}), returning current candidates",
                    ctx.budget_controller.consumed(),
                    ctx.budget_controller.total_budget(),
                ),
                NavigationDecision::Skip,
            );
            return Ok(StageOutcome::complete());
        }

        // Reset Pilot state for new query
        if let Some(ref pilot) = self.pilot {
            pilot.reset();
            debug!(
                "SearchStage: Pilot is available, is_active={}",
                pilot.is_active()
            );
        }

        // Apply budget-aware beam width adjustment
        let effective_beam = ctx
            .budget_controller
            .suggested_beam_width(config.beam_width, ctx.search_iterations);

        info!(
            "Executing search: algorithm={:?}, beam_width={} (budget: {:?}), pilot={}",
            algorithm,
            effective_beam,
            budget_status,
            if self.has_pilot() {
                "enabled"
            } else {
                "disabled"
            }
        );

        ctx.increment_search_iteration();

        // === L1 Cache check: return cached results if available ===
        if ctx.options.enable_cache && ctx.search_iterations <= 1 {
            let scope_fp =
                crate::utils::fingerprint::Fingerprint::from_str(&format!("{:?}", ctx.tree.root()));
            if let Some(cached) = ctx.reasoning_cache.l1_get(&ctx.query, &scope_fp) {
                info!(
                    "L1 cache hit for query, returning {} cached candidates",
                    cached.len()
                );
                ctx.candidates = cached
                    .into_iter()
                    .map(|c| {
                        CandidateNode::new(c.node_id, c.score, c.depth, ctx.tree.is_leaf(c.node_id))
                    })
                    .collect();
                ctx.metrics.cache_hits += 1;
                ctx.record_reasoning(
                    StageName::Search,
                    format!(
                        "L1 cache hit: {} candidates returned from cache",
                        ctx.candidates.len()
                    ),
                    NavigationDecision::ThisIsTheAnswer,
                );
                return Ok(StageOutcome::cont());
            }
            ctx.metrics.cache_misses += 1;
        }

        // === Reasoning Index Quick Match ===
        // Check pre-computed index before running expensive ToC navigation.
        if let Some(ref ridx) = ctx.reasoning_index {
            if let Some(shortcut_candidates) = Self::try_reasoning_shortcut(ridx, ctx) {
                info!(
                    "Reasoning index shortcut match, returning {} candidates",
                    shortcut_candidates.len()
                );
                ctx.candidates = shortcut_candidates;
                ctx.metrics.cache_hits += 1;
                ctx.record_reasoning(
                    StageName::Search,
                    "Reasoning index shortcut: direct path match".to_string(),
                    NavigationDecision::ThisIsTheAnswer,
                );
                return Ok(StageOutcome::cont());
            }
        }

        // === Phase Locate: find relevant subtrees via ToC ===
        // Use depth-1 nodes (root's direct children = top-level sections).
        // level(0) is only the root itself, which is not useful for locating.
        let top_level_nodes: Vec<_> = ctx
            .retrieval_index
            .as_ref()
            .and_then(|idx| idx.level(1))
            .map(|nodes| nodes.to_vec())
            .unwrap_or_else(|| ctx.tree.children(ctx.tree.root()));

        let mut cues = self
            .toc_navigator
            .locate(&ctx.query, &ctx.tree, &top_level_nodes)
            .await;

        // === L2 Cache boost: boost cues whose paths have historical success ===
        let doc_key = format!("{:?}", ctx.tree.root());
        let l2_paths = ctx.reasoning_cache.l2_top_paths(&doc_key, 5);
        if !l2_paths.is_empty() {
            for cue in &mut cues {
                if let Some(node) = ctx.tree.get(cue.root) {
                    let node_path = node.title.as_str();
                    if let Some((_, cached_conf)) = l2_paths
                        .iter()
                        .find(|(path, _)| node_path.contains(path.as_str()) || path.contains(node_path))
                    {
                        // Blend current confidence with historical: 60% current + 40% cached
                        cue.confidence = cue.confidence * 0.6 + cached_conf * 0.4;
                        debug!(
                            "L2 cache boost for '{}': {:.3} → {:.3}",
                            node_path, cue.confidence, cue.confidence
                        );
                    }
                }
            }
        }

        debug!("ToCNavigator returned {} cues", cues.len());

        // Inject structure hints from Analyze stage as high-priority cues
        if !ctx.resolved_path_hints.is_empty() {
            for (hint_text, node_id) in &ctx.resolved_path_hints {
                if ctx.tree.get(*node_id).is_some() {
                    info!("Injecting structure hint '{}' as search cue", hint_text);
                    cues.push(SearchCue {
                        root: *node_id,
                        confidence: 1.0, // Direct match from query structure
                    });
                }
            }
        }

        // === Resolve queries (decomposed or original) ===
        let queries = Self::resolve_queries(ctx);

        // === Phase Traverse + Collect ===
        let (paths, mut candidates) = self.run_search(ctx, &queries, &cues).await;

        // Add cue root nodes as direct candidates.
        // The ToCNavigator already identified these as relevant; they may not
        // be leaf nodes so tree traversal would skip them. This restores the
        // old locate_via_llm behavior where LLM-selected nodes became
        // candidates directly.
        for cue in &cues {
            if let Some(node) = ctx.tree.get(cue.root) {
                candidates.push(CandidateNode::new(
                    cue.root,
                    cue.confidence,
                    node.depth,
                    ctx.tree.is_leaf(cue.root),
                ));
            }
        }

        // Sort by score and deduplicate
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.dedup_by(|a, b| a.node_id == b.node_id);

        ctx.search_paths = paths;
        ctx.candidates = candidates;

        debug!(
            "Search found {} total paths, {} candidates",
            ctx.search_paths.len(),
            ctx.candidates.len()
        );
        for (i, c) in ctx.candidates.iter().enumerate().take(5) {
            if let Some(node) = ctx.tree.get(c.node_id) {
                debug!(
                    "Candidate {}: score={:.3}, title='{}'",
                    i, c.score, node.title
                );
            }
        }

        // Update metrics and budget
        ctx.metrics.search_time_ms += start.elapsed().as_millis() as u64;
        ctx.metrics.nodes_visited += ctx.candidates.len();

        // Update hot node tracker with retrieval results
        if let Some(ref tracker) = ctx.hot_tracker {
            let hits: Vec<(crate::document::NodeId, f32)> = ctx
                .candidates
                .iter()
                .map(|c| (c.node_id, c.score))
                .collect();
            tracker.record_hits(&hits);
        }

        // === L3 Cache boost: use cached strategy scores to refine candidates ===
        for candidate in &mut ctx.candidates {
            if let Some(node) = ctx.tree.get(candidate.node_id) {
                let content_fp = crate::utils::fingerprint::Fingerprint::from_str(&node.content);
                if let Some((cached_score, _strategy)) =
                    ctx.reasoning_cache.l3_get(&content_fp)
                {
                    // Blend: if L3 has a higher score for this node, boost it
                    if cached_score > candidate.score {
                        candidate.score = (candidate.score + cached_score) / 2.0;
                    }
                }
            }
        }
        // Re-sort after L3 boost
        ctx.candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Store L3 scores for future queries
        for candidate in &ctx.candidates {
            if let Some(node) = ctx.tree.get(candidate.node_id) {
                if !node.content.is_empty() {
                    let content_fp =
                        crate::utils::fingerprint::Fingerprint::from_str(&node.content);
                    ctx.reasoning_cache.l3_store(
                        content_fp,
                        candidate.score,
                        ctx.selected_strategy
                            .map(|s| format!("{:?}", s))
                            .unwrap_or_else(|| "auto".to_string()),
                    );
                }
            }
        }
        // Estimate tokens consumed by this search iteration (content-based heuristic)
        let search_tokens: usize = ctx
            .candidates
            .iter()
            .filter_map(|c| ctx.tree.get(c.node_id).map(|n| n.content.len()))
            .sum::<usize>()
            / 4; // rough: 4 chars ≈ 1 token
        ctx.budget_controller.record_tokens(search_tokens);

        // Store results in L1 cache
        if ctx.options.enable_cache && ctx.search_iterations <= 1 && !ctx.candidates.is_empty() {
            let scope_fp =
                crate::utils::fingerprint::Fingerprint::from_str(&format!("{:?}", ctx.tree.root()));
            let cached: Vec<CachedCandidate> = ctx
                .candidates
                .iter()
                .map(|c| CachedCandidate {
                    node_id: c.node_id,
                    score: c.score,
                    depth: c.depth,
                })
                .collect();
            ctx.reasoning_cache.l1_store(
                &ctx.query,
                scope_fp,
                cached,
                ctx.selected_strategy
                    .map(|s| format!("{:?}", s))
                    .unwrap_or_else(|| "auto".to_string()),
            );
        }

        info!(
            "Search complete: {} candidates (iteration {})",
            ctx.candidates.len(),
            ctx.search_iterations
        );

        // Record reasoning — collect data first to avoid borrow conflicts
        let strategy_str = ctx
            .selected_strategy
            .map(|s| format!("{:?}", s))
            .unwrap_or_else(|| "auto".to_string());
        let search_iterations = ctx.search_iterations;

        let reasoning_data: Vec<(
            String,
            Option<String>,
            f32,
            usize,
            String,
            Vec<ReasoningCandidate>,
        )> = ctx
            .candidates
            .iter()
            .take(5)
            .map(|candidate| {
                let (title, depth) = ctx
                    .tree
                    .get(candidate.node_id)
                    .map(|n| (n.title.clone(), n.depth))
                    .unwrap_or_else(|| ("(unknown)".to_string(), 0));

                let considered: Vec<ReasoningCandidate> = ctx
                    .candidates
                    .iter()
                    .filter(|c| c.node_id != candidate.node_id)
                    .take(5)
                    .filter_map(|c| {
                        ctx.tree.get(c.node_id).map(|n| ReasoningCandidate {
                            node_id: format!("{:?}", c.node_id),
                            title: n.title.clone(),
                            score: c.score,
                        })
                    })
                    .collect();

                let reasoning = format!(
                    "Candidate '{}' (score={:.3}) found via {} search, iteration {}",
                    title,
                    candidate.score,
                    algorithm.name(),
                    search_iterations
                );

                (
                    format!("{:?}", candidate.node_id),
                    Some(title),
                    candidate.score,
                    depth,
                    reasoning,
                    considered,
                )
            })
            .collect();

        for (node_id, title, score, depth, reasoning, considered) in reasoning_data {
            ctx.push_reasoning_step(ReasoningStep {
                stage: StageName::Search,
                node_id: Some(node_id),
                title,
                score,
                decision: if score > 0.7 {
                    NavigationDecision::ThisIsTheAnswer
                } else {
                    NavigationDecision::ExploreMore
                },
                depth,
                reasoning,
                candidates: considered,
                strategy_used: Some(strategy_str.clone()),
                llm_call: None,
                references_followed: Vec::new(),
            });
        }

        Ok(StageOutcome::cont())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_stage_creation() {
        let stage = SearchStage::new();
        assert!(stage.llm_strategy.is_none());
        assert!(!stage.has_pilot());
    }

    #[test]
    fn test_search_stage_dependencies() {
        let stage = SearchStage::new();
        assert_eq!(stage.depends_on(), vec!["plan"]);
    }
}
