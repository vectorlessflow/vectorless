// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Worker agent — document navigation and evidence collection.
//!
//! The Worker is a consuming-self struct implementing [`Agent`]:
//! 1. Bird's-eye: ls(root) for initial overview
//! 2. Navigation planning: LLM generates a plan (keyword hits as context)
//! 3. Navigation loop: LLM → parse → execute → repeat (max N rounds)
//!
//! Dispatched by the Orchestrator, one per document.
//! Returns raw evidence — no answer synthesis. Rerank owns all answer generation.

mod execute;
mod format;
mod navigation;
mod planning;

use tracing::info;

use super::Agent;
use super::config::{DocContext, WorkerConfig, WorkerOutput};
use super::context::FindHit;
use super::events::EventEmitter;
use super::state::WorkerState;
use super::tools::worker as tools;
use crate::error::Error;
use crate::llm::LlmClient;
use crate::query::QueryPlan;
use crate::scoring::bm25::extract_keywords;

use navigation::run_navigation_loop;
use planning::build_plan_prompt;

/// Worker agent — navigates a single document to collect evidence.
///
/// Holds all execution context. Calling [`run()`](Agent::run) consumes self.
pub struct Worker<'a> {
    query: String,
    task: Option<String>,
    ctx: &'a DocContext<'a>,
    config: WorkerConfig,
    llm: LlmClient,
    emitter: EventEmitter,
    query_plan: QueryPlan,
}

impl<'a> Worker<'a> {
    /// Create a new Worker.
    pub fn new(
        query: &str,
        task: Option<&str>,
        ctx: &'a DocContext<'a>,
        config: WorkerConfig,
        llm: LlmClient,
        emitter: EventEmitter,
        query_plan: QueryPlan,
    ) -> Self {
        Self {
            query: query.to_string(),
            task: task.map(|s| s.to_string()),
            ctx,
            config,
            llm,
            emitter,
            query_plan,
        }
    }
}

impl<'a> Agent for Worker<'a> {
    type Output = WorkerOutput;

    fn name(&self) -> &str {
        "worker"
    }

    async fn run(self) -> crate::error::Result<WorkerOutput> {
        let Worker {
            query,
            task,
            ctx,
            config,
            llm,
            emitter,
            query_plan,
        } = self;
        let task_ref = task.as_deref();

        let intent_context = format!("{} — {}", query_plan.intent, query_plan.strategy_hint);

        emitter.emit_worker_started(ctx.doc_name, task_ref, config.max_rounds);

        info!(
            doc = ctx.doc_name,
            task = task_ref.unwrap_or("(full query)"),
            max_rounds = config.max_rounds,
            max_llm_calls = config.max_llm_calls,
            "Worker starting"
        );

        let mut llm_calls: u32 = 0;

        // Gather keyword hits as context for LLM planning (not routing rules)
        let keywords = extract_keywords(&query);
        let index_hits: Vec<FindHit> = ctx.find_all(&keywords);
        if !index_hits.is_empty() {
            tracing::debug!(
                doc = ctx.doc_name,
                hit_count = index_hits.len(),
                "ReasoningIndex keyword hits available for planning"
            );
        }

        // --- Phase 1: Bird's-eye view ---
        let mut state = WorkerState::new(ctx.root(), config.max_rounds);
        let ls_result = tools::ls(ctx, &state);
        state.set_feedback(ls_result.feedback);

        // --- Phase 1.5: Navigation planning ---
        if state.remaining > 0 && (config.max_llm_calls == 0 || llm_calls < config.max_llm_calls) {
            info!(doc = ctx.doc_name, "Generating navigation plan...");
            let plan_prompt = build_plan_prompt(
                &query,
                task_ref,
                &state.last_feedback,
                ctx.doc_name,
                &index_hits,
                ctx,
                query_plan.intent,
            );
            let plan_output = llm
                .complete(&plan_prompt.0, &plan_prompt.1)
                .await
                .map_err(|e| Error::LlmReasoning {
                    stage: "worker/plan".to_string(),
                    detail: format!("Navigation plan LLM call failed: {e}"),
                })?;
            llm_calls += 1;
            let plan_text = plan_output.trim().to_string();
            if !plan_text.is_empty() {
                info!(
                    doc = ctx.doc_name,
                    plan = %plan_text,
                    "Navigation plan generated"
                );
                emitter.emit_worker_plan_generated(ctx.doc_name, plan_text.len());
                state.plan = plan_text;
                state.plan_generated = true;
            }
        }

        // --- Phase 2: Navigation loop ---
        run_navigation_loop(
            &query,
            task_ref,
            ctx,
            &config,
            &llm,
            &mut state,
            &emitter,
            &index_hits,
            &intent_context,
            &mut llm_calls,
        )
        .await?;

        let budget_exhausted = state.remaining == 0
            || (config.max_llm_calls > 0 && llm_calls >= config.max_llm_calls);

        let output = state.into_worker_output(llm_calls, budget_exhausted, ctx.doc_name);

        emitter.emit_worker_done(
            ctx.doc_name,
            output.evidence.len(),
            output.metrics.rounds_used,
            output.metrics.llm_calls,
            output.metrics.budget_exhausted,
            output.metrics.plan_generated,
        );

        info!(
            doc = ctx.doc_name,
            evidence = output.evidence.len(),
            rounds = output.metrics.rounds_used,
            llm_calls = output.metrics.llm_calls,
            "Worker complete"
        );

        Ok(output)
    }
}

#[cfg(test)]
mod truncation_tests {
    /// Verify that truncating feedback with multi-byte UTF-8 characters
    /// never panics. This mirrors the truncation logic in the navigation loop.
    #[test]
    fn test_utf8_safe_truncation_ascii() {
        let feedback = "a".repeat(200);
        let boundary = feedback.ceil_char_boundary(120);
        let truncated = &feedback[..boundary];
        assert!(truncated.len() <= 123); // 120 + "..." fits
        assert!(truncated.is_char_boundary(truncated.len()));
    }

    #[test]
    fn test_utf8_safe_truncation_multibyte() {
        // Each '中' is 3 bytes in UTF-8
        let feedback = "中文反馈内容测试截断安全".repeat(20);
        assert!(feedback.len() > 120);
        let boundary = feedback.ceil_char_boundary(120);
        let truncated = &feedback[..boundary];
        assert!(truncated.len() <= 120);
        assert!(truncated.is_char_boundary(truncated.len()));
    }

    #[test]
    fn test_utf8_safe_truncation_emoji() {
        // Emojis are 4 bytes each
        let feedback = "🦀🎉🚀".repeat(50);
        assert!(feedback.len() > 120);
        let boundary = feedback.ceil_char_boundary(120);
        let truncated = &feedback[..boundary];
        assert!(truncated.len() <= 120);
        assert!(truncated.is_char_boundary(truncated.len()));
    }

    #[test]
    fn test_utf8_safe_truncation_short_string() {
        // String shorter than limit — no truncation needed
        let feedback = "short feedback".to_string();
        let boundary = feedback.ceil_char_boundary(120);
        assert_eq!(boundary, feedback.len());
    }
}
