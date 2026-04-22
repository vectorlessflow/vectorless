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
mod planning;

use tracing::{debug, info};

use super::Agent;
use super::command::Command;
use super::config::{DocContext, Step, WorkerConfig, WorkerOutput};
use super::context::FindHit;
use super::events::EventEmitter;
use super::prompts::{NavigationParams, worker_dispatch, worker_navigation};
use super::state::WorkerState;
use super::tools::worker as tools;
use crate::error::Error;
use crate::llm::LlmClient;
use crate::query::QueryPlan;
use crate::scoring::bm25::extract_keywords;

use execute::{execute_command, parse_and_detect_failure};
use format::format_visited_titles;
use planning::{build_plan_prompt, build_replan_prompt, format_keyword_hints};

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
        let max_llm = config.max_llm_calls;

        macro_rules! llm_budget_exhausted {
            () => {
                max_llm > 0 && llm_calls >= max_llm
            };
        }

        // Gather keyword hits as context for LLM planning (not routing rules)
        let keywords = extract_keywords(&query);
        let index_hits: Vec<FindHit> = ctx.find_all(&keywords);
        if !index_hits.is_empty() {
            debug!(
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
        if state.remaining > 0 && !llm_budget_exhausted!() {
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
        let use_dispatch_prompt = task_ref.is_some();
        let keyword_hints = format_keyword_hints(&index_hits, ctx);

        loop {
            if state.remaining == 0 {
                info!(doc = ctx.doc_name, "Navigation budget exhausted");
                break;
            }
            if llm_budget_exhausted!() {
                info!(
                    doc = ctx.doc_name,
                    llm_calls, max_llm, "LLM call budget exhausted"
                );
                break;
            }

            // Build prompt
            let (system, user) = if use_dispatch_prompt && state.remaining == config.max_rounds {
                worker_dispatch(&super::prompts::WorkerDispatchParams {
                    original_query: &query,
                    task: task_ref.unwrap_or(&query),
                    doc_name: ctx.doc_name,
                    breadcrumb: &state.path_str(),
                })
            } else {
                let visited_titles = format_visited_titles(&state, ctx);
                worker_navigation(&NavigationParams {
                    query: &query,
                    task: task_ref,
                    breadcrumb: &state.path_str(),
                    evidence_summary: &state.evidence_summary(),
                    missing_info: &state.missing_info,
                    last_feedback: &state.last_feedback,
                    remaining: state.remaining,
                    max_rounds: state.max_rounds,
                    history: &state.history_text(),
                    visited_titles: &visited_titles,
                    plan: &state.plan,
                    intent_context: &intent_context,
                    keyword_hints: &keyword_hints,
                })
            };

            // LLM decision
            let round_num = config.max_rounds - state.remaining + 1;
            let round_start = std::time::Instant::now();
            info!(
                doc = ctx.doc_name,
                round = round_num,
                max_rounds = config.max_rounds,
                "Navigation round: calling LLM..."
            );
            let llm_output =
                llm.complete(&system, &user)
                    .await
                    .map_err(|e| Error::LlmReasoning {
                        stage: "worker/navigation".to_string(),
                        detail: format!("Nav loop LLM call failed (round {round_num}): {e}"),
                    })?;
            llm_calls += 1;

            // Parse command
            if llm_output.trim().len() < 2 {
                tracing::warn!(
                    doc = ctx.doc_name,
                    round = config.max_rounds - state.remaining + 1,
                    response = llm_output.trim(),
                    "LLM response unusually short"
                );
            }
            let (command, is_parse_failure) = parse_and_detect_failure(&llm_output);
            if is_parse_failure {
                let raw_preview = if llm_output.trim().len() > 200 {
                    format!("{}...", &llm_output.trim()[..200])
                } else {
                    llm_output.trim().to_string()
                };
                state.last_feedback = format!(
                    "Your output was not recognized as a valid command:\n\"{}\"\n\n\
                     Please output exactly one command (ls, cd, cat, head, find, findtree, grep, wc, pwd, check, or done).",
                    raw_preview
                );
                state.push_history("(unrecognized) → parse failure".to_string());
                continue;
            }

            debug!(doc = ctx.doc_name, ?command, "Parsed command");

            let is_check = matches!(command, Command::Check);

            // Execute
            let step = execute_command(
                &command,
                ctx,
                &mut state,
                &query,
                &llm,
                &mut llm_calls,
                &emitter,
            )
            .await;

            // Dynamic re-planning after insufficient check
            if is_check
                && !state.missing_info.is_empty()
                && state.remaining >= 3
                && !llm_budget_exhausted!()
            {
                let missing = state.missing_info.clone();
                info!(doc = ctx.doc_name, missing = %missing, "Re-planning navigation...");
                let replan = build_replan_prompt(&query, task_ref, &state, ctx);
                let new_plan =
                    llm.complete(&replan.0, &replan.1)
                        .await
                        .map_err(|e| Error::LlmReasoning {
                            stage: "worker/replan".to_string(),
                            detail: format!("Re-plan LLM call failed: {e}"),
                        })?;
                llm_calls += 1;
                let plan_text = new_plan.trim().to_string();
                if !plan_text.is_empty() {
                    info!(
                        doc = ctx.doc_name,
                        plan = %plan_text,
                        "Re-plan generated"
                    );
                    emitter.emit_worker_replan(ctx.doc_name, &missing, plan_text.len());
                    state.plan = plan_text;
                }
                state.missing_info.clear();
            } else if is_check && !state.missing_info.is_empty() {
                state.plan.clear();
                state.missing_info.clear();
            }

            // Emit round event
            let cmd_str = format!("{:?}", command);
            let success = !matches!(step, Step::ForceDone(_));
            let round_elapsed = round_start.elapsed().as_millis() as u64;
            emitter.emit_worker_round(ctx.doc_name, round_num, &cmd_str, success, round_elapsed);

            let feedback_preview = if state.last_feedback.len() > 120 {
                let boundary = state.last_feedback.ceil_char_boundary(120);
                format!("{}...", &state.last_feedback[..boundary])
            } else {
                state.last_feedback.clone()
            };
            state.push_history(format!("{} → {}", cmd_str, feedback_preview));

            // Check termination
            match step {
                Step::Done => {
                    info!(
                        doc = ctx.doc_name,
                        evidence = state.evidence.len(),
                        "Navigation done"
                    );
                    break;
                }
                Step::ForceDone(reason) => {
                    info!(doc = ctx.doc_name, reason = %reason, "Forced done");
                    break;
                }
                Step::Continue => {
                    if !is_check {
                        state.dec_round();
                    }
                }
            }
        }

        let budget_exhausted = state.remaining == 0 || llm_budget_exhausted!();

        // Worker returns raw evidence — no synthesis.
        // The Orchestrator owns the single synthesis/fusion point via rerank::process.
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
