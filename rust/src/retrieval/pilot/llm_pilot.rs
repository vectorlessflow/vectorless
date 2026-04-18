// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM-based Pilot implementation.
//!
//! This module provides the main Pilot implementation that uses LLM
//! for semantic navigation guidance.

use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::document::{DocumentTree, NodeId};
use crate::llm::memo::{MemoKey, MemoStore, MemoValue};
use crate::llm::{LlmClient, LlmExecutor};
use crate::utils::fingerprint::Fingerprint;

use super::budget::BudgetController;
use super::builder::ContextBuilder;
use super::config::PilotConfig;
use super::decision::{InterventionPoint, PilotDecision};
use super::feedback::{FeedbackRecord, FeedbackStore, PilotLearner};
use super::parser::ResponseParser;
use super::prompts::PromptBuilder;
use super::r#trait::{Pilot, SearchState};

/// LLM-based Pilot implementation.
///
/// Uses an LLM client to provide semantic navigation guidance
/// at key decision points during tree search.
///
/// # Architecture
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────┐
/// │                         LlmPilot                             │
/// ├─────────────────────────────────────────────────────────────┤
/// │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
/// │  │ Context     │  │ Prompt      │  │ Response    │         │
/// │  │ Builder     │─▶│ Builder     │─▶│ Parser      │         │
/// │  └─────────────┘  └─────────────┘  └─────────────┘         │
/// │                                                              │
/// │  ┌─────────────┐  ┌───────────────────────┐                │
/// │  │ Budget      │  │ LlmExecutor           │                │
/// │  │ Controller  │  │ (throttle+retry+fall) │                │
/// │  └─────────────┘  └───────────────────────┘                │
/// │                                                              │
/// │  ┌─────────────┐  ┌───────────────────────┐                │
/// │  │ Memo        │  │ (cache LLM decisions) │                │
/// │  │ Store       │  │                       │                │
/// │  └─────────────┘  └───────────────────────┘                │
/// └─────────────────────────────────────────────────────────────┘
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::pilot::{LlmPilot, PilotConfig};
/// use vectorless::llm::{LlmClient, LlmExecutor};
///
/// let client = LlmClient::for_model("gpt-4o-mini");
/// let pilot = LlmPilot::new(client, PilotConfig::default());
///
/// // Or with executor for unified throttle/retry/fallback
/// let executor = LlmExecutor::for_model("gpt-4o-mini");
/// let pilot = LlmPilot::with_executor(executor, PilotConfig::default());
///
/// // Use in search
/// if pilot.should_intervene(&state) {
///     let decision = pilot.decide(&state).await;
/// }
/// ```
pub struct LlmPilot {
    /// LLM client for making requests (fallback when no executor).
    client: LlmClient,
    /// LLM executor with unified throttle/retry/fallback (optional).
    executor: Option<Arc<LlmExecutor>>,
    /// Pilot configuration.
    config: PilotConfig,
    /// Budget controller for per-level call tracking.
    budget: BudgetController,
    /// Shared pipeline budget — the primary budget source when set.
    /// When available, Pilot checks this before making LLM calls and
    /// records token consumption here.
    pipeline_budget:
        parking_lot::RwLock<Option<Arc<crate::retrieval::pipeline::RetrievalBudgetController>>>,
    /// Context builder.
    context_builder: ContextBuilder,
    /// Prompt builder.
    prompt_builder: PromptBuilder,
    /// Response parser.
    response_parser: ResponseParser,
    /// Feedback learner for improving decisions (optional).
    learner: Option<Arc<PilotLearner>>,
    /// Memo store for caching decisions (optional).
    memo_store: Option<MemoStore>,
}

impl std::fmt::Debug for LlmPilot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmPilot")
            .field("config", &self.config)
            .field("budget", &self.budget.usage())
            .finish()
    }
}

impl LlmPilot {
    /// Create a new LLM-based Pilot.
    pub fn new(client: LlmClient, config: PilotConfig) -> Self {
        let budget = BudgetController::new(config.budget.clone());
        let token_budget = config.budget.max_tokens_per_call;

        Self {
            client,
            executor: None,
            config,
            budget,
            pipeline_budget: parking_lot::RwLock::new(None),
            context_builder: ContextBuilder::new(token_budget),
            prompt_builder: PromptBuilder::new(),
            response_parser: ResponseParser::new(),
            learner: None,
            memo_store: None,
        }
    }

    /// Create a Pilot with LlmExecutor for unified throttle/retry/fallback.
    pub fn with_executor(executor: LlmExecutor, config: PilotConfig) -> Self {
        let budget = BudgetController::new(config.budget.clone());
        let token_budget = config.budget.max_tokens_per_call;
        // Create a fallback client for backwards compatibility
        let client = LlmClient::for_model(&executor.config().model);

        Self {
            client,
            executor: Some(Arc::new(executor)),
            config,
            budget,
            pipeline_budget: parking_lot::RwLock::new(None),
            context_builder: ContextBuilder::new(token_budget),
            prompt_builder: PromptBuilder::new(),
            response_parser: ResponseParser::new(),
            learner: None,
            memo_store: None,
        }
    }

    /// Create a Pilot with shared executor (for sharing throttle/fallback across pilots).
    pub fn with_shared_executor(executor: Arc<LlmExecutor>, config: PilotConfig) -> Self {
        let budget = BudgetController::new(config.budget.clone());
        let token_budget = config.budget.max_tokens_per_call;
        let client = LlmClient::for_model(&executor.config().model);

        Self {
            client,
            executor: Some(executor),
            config,
            budget,
            pipeline_budget: parking_lot::RwLock::new(None),
            context_builder: ContextBuilder::new(token_budget),
            prompt_builder: PromptBuilder::new(),
            response_parser: ResponseParser::new(),
            learner: None,
            memo_store: None,
        }
    }

    /// Create with custom builders.
    pub fn with_builders(
        client: LlmClient,
        config: PilotConfig,
        context_builder: ContextBuilder,
        prompt_builder: PromptBuilder,
    ) -> Self {
        let budget = BudgetController::new(config.budget.clone());

        Self {
            client,
            executor: None,
            config,
            budget,
            pipeline_budget: parking_lot::RwLock::new(None),
            context_builder,
            prompt_builder,
            response_parser: ResponseParser::new(),
            learner: None,
            memo_store: None,
        }
    }

    /// Add an executor to an existing pilot.
    pub fn with_executor_mut(mut self, executor: LlmExecutor) -> Self {
        self.executor = Some(Arc::new(executor));
        self
    }

    /// Add a feedback learner to the pilot.
    pub fn with_learner(mut self, learner: Arc<PilotLearner>) -> Self {
        self.learner = Some(learner);
        self
    }

    /// Add a feedback learner from a feedback store.
    pub fn with_feedback_store(mut self, store: Arc<FeedbackStore>) -> Self {
        self.learner = Some(Arc::new(PilotLearner::new(store)));
        self
    }

    /// Add a memo store for caching decisions.
    ///
    /// When enabled, the pilot will cache LLM decisions based on
    /// context fingerprints, avoiding redundant API calls for
    /// similar navigation scenarios.
    pub fn with_memo_store(mut self, store: MemoStore) -> Self {
        self.memo_store = Some(store);
        self
    }

    /// Set the shared pipeline budget controller.
    ///
    /// When set, this becomes the primary budget gate for LLM calls.
    /// The Pilot's own BudgetController still tracks per-level call counts,
    /// but token consumption is recorded against the pipeline budget.
    /// Call this at query time (not construction time) since the pipeline
    /// budget is created per-query.
    pub fn set_pipeline_budget(
        &self,
        budget: Arc<crate::retrieval::pipeline::RetrievalBudgetController>,
    ) {
        *self.pipeline_budget.write() = Some(budget);
    }

    /// Check if using LlmExecutor (unified throttle/retry/fallback).
    pub fn has_executor(&self) -> bool {
        self.executor.is_some()
    }

    /// Check if using feedback learner.
    pub fn has_learner(&self) -> bool {
        self.learner.is_some()
    }

    /// Check if using memo store.
    pub fn has_memo_store(&self) -> bool {
        self.memo_store.is_some()
    }

    /// Get the feedback learner (if any).
    pub fn learner(&self) -> Option<&PilotLearner> {
        self.learner.as_deref()
    }

    /// Get the memo store (if any).
    pub fn memo_store(&self) -> Option<&MemoStore> {
        self.memo_store.as_ref()
    }

    /// Record feedback for a decision.
    pub fn record_feedback(&self, record: FeedbackRecord) {
        if let Some(ref learner) = self.learner {
            let decision_id = record.decision_id;
            learner.store().record(record);
            debug!("Recorded feedback for decision {:?}", decision_id);
        }
    }

    /// Compute a cache key for a pilot decision.
    fn compute_cache_key(
        &self,
        context: &super::builder::PilotContext,
        _point: InterventionPoint,
    ) -> Option<MemoKey> {
        let _store = self.memo_store.as_ref()?;

        // Build a fingerprint from the context using available methods
        let context_str = context.to_string();
        let context_fp = Fingerprint::from_str(&context_str);
        let query_fp = Fingerprint::from_str(&context.query_section);

        Some(MemoKey::pilot_decision(&context_fp, &query_fp))
    }

    /// Check if budget allows LLM calls.
    ///
    /// Checks the shared pipeline budget first (if set), then falls back
    /// to the Pilot's own per-call budget.
    fn has_budget(&self) -> bool {
        // Primary: check pipeline budget
        if let Some(ref pb) = *self.pipeline_budget.read() {
            if pb.status().should_stop() {
                return false;
            }
        }
        // Secondary: check Pilot's own call-level budget
        self.budget.can_call()
    }

    /// Check if scores are too close (algorithm uncertain).
    fn scores_are_close(&self, state: &SearchState<'_>) -> bool {
        // Use the config's score_gap_threshold with the state's best_score
        // If best_score is low, consider scores as close
        state.candidates.len() >= 2
            && state.best_score < self.config.intervention.score_gap_threshold
    }

    /// Determine the intervention point type.
    fn get_intervention_point(&self, state: &SearchState<'_>) -> InterventionPoint {
        if state.is_at_root() || state.iteration == 0 {
            InterventionPoint::Start
        } else if state.is_backtracking {
            InterventionPoint::Backtrack
        } else if state.is_fork_point() {
            InterventionPoint::Fork
        } else {
            InterventionPoint::Evaluate
        }
    }

    /// Make an LLM call and return the decision.
    async fn call_llm(
        &self,
        point: InterventionPoint,
        context: &super::builder::PilotContext,
        candidates: &[super::parser::CandidateInfo],
    ) -> PilotDecision {
        // Check memo cache first
        if let Some(ref store) = self.memo_store {
            if let Some(cache_key) = self.compute_cache_key(context, point) {
                if let Some(cached) = store.get(&cache_key) {
                    if let MemoValue::PilotDecision(decision_value) = cached {
                        debug!("Memo cache hit for pilot decision at {:?}", point);
                        // Convert cached value back to PilotDecision
                        let decision =
                            self.cached_value_to_decision(decision_value, candidates, point);
                        return decision;
                    }
                }
            }
        }

        // Build prompt
        let prompt = self.prompt_builder.build(point, context);

        // Check if we can afford this call
        if !self.budget.can_afford(prompt.estimated_tokens) {
            warn!(
                "Budget cannot afford LLM call (estimated: {} tokens)",
                prompt.estimated_tokens
            );
            return self.default_decision(candidates, point);
        }

        // Get learner adjustment if available
        let adjustment = if let Some(ref learner) = self.learner {
            let query_hash = context.query_hash();
            let path_hash = context.path_hash();
            Some(learner.get_adjustment(point, query_hash, path_hash))
        } else {
            None
        };

        // Check if learner suggests skipping intervention
        if let Some(ref adj) = adjustment {
            if adj.skip_intervention {
                debug!("Learner suggests skipping intervention (low historical accuracy)");
                return self.default_decision(candidates, point);
            }
        }

        debug!(
            "Calling LLM for {:?} point (estimated: {} tokens)",
            point, prompt.estimated_tokens
        );

        // Make LLM call -use executor if available, otherwise use client directly
        let result = if let Some(ref executor) = self.executor {
            // Use LlmExecutor for unified throttle/retry/fallback
            executor.complete(&prompt.system, &prompt.user).await
        } else {
            // Fallback to direct client call
            self.client.complete(&prompt.system, &prompt.user).await
        };

        match result {
            Ok(response) => {
                // Record usage (estimate output tokens)
                let output_tokens = self.estimate_tokens(&response);
                let total_tokens = prompt.estimated_tokens + output_tokens;
                self.budget
                    .record_usage(prompt.estimated_tokens, output_tokens, 0);

                // Also record in pipeline budget if shared
                if let Some(ref pb) = *self.pipeline_budget.read() {
                    pb.record_tokens(total_tokens);
                }

                // Parse response
                let mut decision = self.response_parser.parse(&response, candidates, point);

                // Apply learner adjustment if available
                if let Some(ref adj) = adjustment {
                    decision.confidence =
                        (decision.confidence + adj.confidence_delta as f32).clamp(0.0, 1.0);
                    debug!(
                        "Applied learner adjustment: confidence_delta={:.2}, algorithm_weight={:.2}",
                        adj.confidence_delta, adj.algorithm_weight
                    );
                }

                info!(
                    "LLM decision: direction={:?}, confidence={:.2}, candidates={}",
                    std::mem::discriminant(&decision.direction),
                    decision.confidence,
                    decision.ranked_candidates.len()
                );

                // Cache the decision
                if let Some(ref store) = self.memo_store {
                    if let Some(cache_key) = self.compute_cache_key(context, point) {
                        let decision_value = self.decision_to_cached_value(&decision);
                        let tokens_saved = prompt.estimated_tokens as u64 + output_tokens as u64;
                        store.put_with_tokens(
                            cache_key,
                            MemoValue::PilotDecision(decision_value),
                            tokens_saved,
                        );
                        debug!("Memo cache stored for pilot decision at {:?}", point);
                    }
                }

                decision
            }
            Err(e) => {
                warn!("LLM call failed: {}", e);
                self.default_decision(candidates, point)
            }
        }
    }

    /// Convert a PilotDecision to a cacheable value.
    fn decision_to_cached_value(
        &self,
        decision: &PilotDecision,
    ) -> crate::llm::memo::PilotDecisionValue {
        crate::llm::memo::PilotDecisionValue {
            selected_idx: decision
                .ranked_candidates
                .first()
                .map(|c| c.node_id.0.into())
                .unwrap_or(0),
            confidence: decision.confidence,
            reasoning: decision.reasoning.clone(),
        }
    }

    /// Convert a cached value back to a PilotDecision.
    fn cached_value_to_decision(
        &self,
        value: crate::llm::memo::PilotDecisionValue,
        candidates: &[super::parser::CandidateInfo],
        point: InterventionPoint,
    ) -> PilotDecision {
        let ranked = candidates
            .iter()
            .enumerate()
            .map(|(i, c)| super::decision::RankedCandidate {
                node_id: c.node_id,
                score: if i == value.selected_idx {
                    1.0
                } else {
                    0.5 / (i + 1) as f32
                },
                reason: None,
            })
            .collect();

        PilotDecision {
            ranked_candidates: ranked,
            direction: super::decision::SearchDirection::GoDeeper {
                reason: "Cached decision".to_string(),
            },
            confidence: value.confidence,
            reasoning: value.reasoning,
            intervention_point: point,
        }
    }

    /// Create a default decision when LLM fails.
    fn default_decision(
        &self,
        candidates: &[super::parser::CandidateInfo],
        point: InterventionPoint,
    ) -> PilotDecision {
        let ranked = candidates
            .iter()
            .enumerate()
            .map(|(i, c)| super::decision::RankedCandidate {
                node_id: c.node_id,
                score: 1.0 / (i + 1) as f32,
                reason: None,
            })
            .collect();

        PilotDecision {
            ranked_candidates: ranked,
            direction: super::decision::SearchDirection::GoDeeper {
                reason: "Default decision (LLM unavailable)".to_string(),
            },
            confidence: 0.0,
            reasoning: "LLM call failed or budget exhausted".to_string(),
            intervention_point: point,
        }
    }

    /// Estimate token count for a string.
    fn estimate_tokens(&self, text: &str) -> usize {
        let char_count = text.chars().count();
        let chinese_count = text
            .chars()
            .filter(|c| ('\u{4E00}'..='\u{9FFF}').contains(c))
            .count();
        let english_count = char_count - chinese_count;

        (chinese_count as f32 / 1.5 + english_count as f32 / 4.0).ceil() as usize
    }
}

#[async_trait]
impl Pilot for LlmPilot {
    fn name(&self) -> &str {
        "llm_pilot"
    }

    fn should_intervene(&self, state: &SearchState<'_>) -> bool {
        // Check mode
        if !self.config.mode.uses_llm() {
            return false;
        }

        // Check budget
        if !self.has_budget() {
            debug!("Budget exhausted, skipping intervention");
            return false;
        }

        let intervention = &self.config.intervention;

        // Condition 1: Fork point with enough candidates
        if state.candidates.len() > intervention.fork_threshold {
            debug!(
                "Intervening: fork point with {} candidates",
                state.candidates.len()
            );
            return true;
        }

        // Condition 2: Scores are too close (algorithm uncertain)
        if self.scores_are_close(state) {
            debug!("Intervening: scores are close");
            return true;
        }

        // Condition 3: Low confidence (best score too low)
        if intervention.is_low_confidence(state.best_score) {
            debug!(
                "Intervening: low confidence (best_score={:.2})",
                state.best_score
            );
            return true;
        }

        // Condition 4: Backtracking and guide_at_backtrack is enabled
        if state.is_backtracking && self.config.guide_at_backtrack {
            debug!("Intervening: backtracking");
            return true;
        }

        false
    }

    async fn decide(&self, state: &SearchState<'_>) -> PilotDecision {
        let point = self.get_intervention_point(state);

        // Build context
        let context = self.context_builder.build(state);

        // Build candidate info with titles
        let candidate_info: Vec<super::parser::CandidateInfo> = state
            .candidates
            .iter()
            .enumerate()
            .filter_map(|(i, &node_id)| {
                state
                    .tree
                    .get(node_id)
                    .map(|node| super::parser::CandidateInfo {
                        node_id,
                        title: node.title.clone(),
                        index: i,
                    })
            })
            .collect();

        // Make LLM call
        self.call_llm(point, &context, &candidate_info).await
    }

    async fn guide_start(
        &self,
        tree: &DocumentTree,
        query: &str,
        start_node: NodeId,
    ) -> Option<PilotDecision> {
        // Check if guide_at_start is enabled
        if !self.config.guide_at_start {
            return None;
        }

        // Check budget
        if !self.has_budget() {
            debug!("Budget exhausted, cannot guide start");
            return None;
        }

        // Build start context
        let context = self.context_builder.build_start_context(tree, query);

        // Get start_node's children as candidates (NOT root's children)
        let node_ids = tree.children(start_node);
        if node_ids.is_empty() {
            debug!("Start node has no children, no guidance needed");
            return None;
        }

        // Build CandidateInfo with titles
        let candidates: Vec<super::parser::CandidateInfo> = node_ids
            .iter()
            .enumerate()
            .filter_map(|(i, &node_id)| {
                tree.get(node_id).map(|node| super::parser::CandidateInfo {
                    node_id,
                    title: node.title.clone(),
                    index: i,
                })
            })
            .collect();

        // Make LLM call
        let decision = self
            .call_llm(InterventionPoint::Start, &context, &candidates)
            .await;

        info!(
            "Pilot start guidance: confidence={:.2}, candidates={}",
            decision.confidence,
            decision.ranked_candidates.len()
        );

        Some(decision)
    }

    async fn guide_backtrack(&self, state: &SearchState<'_>) -> Option<PilotDecision> {
        // Check if guide_at_backtrack is enabled
        if !self.config.guide_at_backtrack {
            return None;
        }

        // Check budget
        if !self.has_budget() {
            return None;
        }

        // Build backtrack context
        let context = self
            .context_builder
            .build_backtrack_context(state, state.path);

        // Build CandidateInfo
        let candidates: Vec<super::parser::CandidateInfo> = state
            .candidates
            .iter()
            .enumerate()
            .filter_map(|(i, &node_id)| {
                state
                    .tree
                    .get(node_id)
                    .map(|node| super::parser::CandidateInfo {
                        node_id,
                        title: node.title.clone(),
                        index: i,
                    })
            })
            .collect();

        // Make LLM call
        Some(
            self.call_llm(InterventionPoint::Backtrack, &context, &candidates)
                .await,
        )
    }

    async fn binary_prune(&self, state: &SearchState<'_>) -> Option<Vec<NodeId>> {
        if !self.has_budget() {
            debug!("Budget exhausted, cannot binary prune");
            return None;
        }

        let context = self.context_builder.build(state);

        let candidate_info: Vec<super::parser::CandidateInfo> = state
            .candidates
            .iter()
            .enumerate()
            .filter_map(|(i, &node_id)| {
                state
                    .tree
                    .get(node_id)
                    .map(|node| super::parser::CandidateInfo {
                        node_id,
                        title: node.title.clone(),
                        index: i,
                    })
            })
            .collect();

        let decision = self
            .call_llm(InterventionPoint::Prune, &context, &candidate_info)
            .await;

        // Extract relevant node IDs from ranked candidates (score > 0.5 means relevant)
        let relevant: Vec<NodeId> = decision
            .ranked_candidates
            .iter()
            .filter(|c| c.score > 0.5)
            .map(|c| c.node_id)
            .collect();

        if relevant.is_empty() {
            debug!("Binary prune: LLM marked no candidates as relevant");
            return None;
        }

        debug!(
            "Binary prune: {} of {} candidates marked relevant",
            relevant.len(),
            state.candidates.len()
        );

        Some(relevant)
    }

    fn config(&self) -> &PilotConfig {
        &self.config
    }

    fn is_active(&self) -> bool {
        self.config.mode.uses_llm() && self.has_budget()
    }

    fn reset(&self) {
        self.budget.reset();
        *self.pipeline_budget.write() = None;
        debug!("LlmPilot reset for new query");
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::NodeId;
    use indextree::Arena;

    fn create_test_node_ids(count: usize) -> Vec<NodeId> {
        let mut arena = Arena::new();
        let mut ids = Vec::new();
        for i in 0..count {
            let node = crate::document::TreeNode {
                title: format!("Node {}", i),
                structure: String::new(),
                content: String::new(),
                summary: String::new(),
                depth: 0,
                start_index: 1,
                end_index: 1,
                start_page: None,
                end_page: None,
                node_id: None,
                physical_index: None,
                token_count: None,
                references: Vec::new(),
                routing_keywords: Vec::new(),
                question_hints: Vec::new(),
            };
            ids.push(NodeId(arena.new_node(node)));
        }
        ids
    }

    #[test]
    fn test_llm_pilot_creation() {
        let client = LlmClient::for_model("gpt-4o-mini");
        let config = PilotConfig::default();
        let pilot = LlmPilot::new(client, config);

        assert_eq!(pilot.name(), "llm_pilot");
        assert!(pilot.is_active());
    }

    #[test]
    fn test_llm_pilot_algorithm_only_mode() {
        let client = LlmClient::for_model("gpt-4o-mini");
        let config = PilotConfig::algorithm_only();
        let pilot = LlmPilot::new(client, config);

        assert!(!pilot.config().mode.uses_llm());
    }

    #[test]
    fn test_llm_pilot_budget_exhausted() {
        let client = LlmClient::for_model("gpt-4o-mini");
        let config = PilotConfig::default();
        let pilot = LlmPilot::new(client, config);

        // Exhaust budget
        pilot.budget.record_usage(3000, 500, 0);

        assert!(!pilot.has_budget());
    }

    #[test]
    fn test_reset() {
        let client = LlmClient::for_model("gpt-4o-mini");
        let config = PilotConfig::default();
        let pilot = LlmPilot::new(client, config);

        // Use some budget
        pilot.budget.record_usage(100, 50, 0);
        assert!(pilot.budget.total_tokens() > 0);

        // Reset
        pilot.reset();
        assert_eq!(pilot.budget.total_tokens(), 0);
    }
}
