// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Worker agent — document navigation and evidence collection.
//!
//! The Worker is a consuming-self struct implementing [`Agent`]:
//! 1. Fast path: keyword lookup → direct hit?
//! 2. Bird's-eye: ls(root) for initial overview
//! 3. Navigation loop: LLM → parse → execute → repeat (max N rounds)
//! 4. Answer synthesis: LLM generates final answer from evidence
//!
//! Dispatched by the Orchestrator, one per document.

mod execute;
mod fast_path;
mod format;
mod planning;

use tracing::{debug, info, warn};

use crate::llm::LlmClient;
use super::Agent;
use super::command::Command;
use super::config::{DocContext, Output, Step, WorkerConfig};
use super::context::FindHit;
use super::events::EventEmitter;
use super::prompts::{
    NavigationParams, worker_dispatch, worker_navigation,
};
use super::state::WorkerState;
use super::tools::worker as tools;

use execute::{execute_command, parse_and_detect_failure};
use fast_path::{FastPathResult, fast_path};
use format::format_visited_titles;
use planning::{build_plan_prompt, build_replan_prompt};

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
    ) -> Self {
        Self {
            query: query.to_string(),
            task: task.map(|s| s.to_string()),
            ctx,
            config,
            llm,
            emitter,
        }
    }
}

impl<'a> Agent for Worker<'a> {
    type Output = Output;

    fn name(&self) -> &str {
        "worker"
    }

    async fn run(self) -> crate::error::Result<Output> {
        let Worker { query, task, ctx, config, llm, emitter } = self;
        let task_ref = task.as_deref();

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
            () => { max_llm > 0 && llm_calls >= max_llm }
        }

        // --- Phase 0: Fast path ---
        let mut preserved_hits: Vec<FindHit> = Vec::new();
        if config.enable_fast_path {
            match fast_path(&query, ctx, &config, &emitter) {
                FastPathResult::Hit(output) => {
                    info!(doc = ctx.doc_name, "Fast path hit — skipping navigation");
                    emitter.emit_worker_done(
                        ctx.doc_name, output.evidence.len(),
                        output.metrics.rounds_used, output.metrics.llm_calls,
                        false, false,
                    );
                    return Ok(output);
                }
                FastPathResult::Miss(hits) => {
                    if !hits.is_empty() {
                        debug!(doc = ctx.doc_name, hit_count = hits.len(), "Fast path miss — preserving hits");
                        preserved_hits = hits;
                    }
                }
            }
        }

        // --- Phase 1: Bird's-eye view + adaptive budget ---
        let doc_depth = ctx.tree.max_depth();
        let adaptive_rounds = adaptive_rounds(config.max_rounds, doc_depth);
        if adaptive_rounds != config.max_rounds {
            info!(
                doc = ctx.doc_name, doc_depth,
                configured_rounds = config.max_rounds, adaptive_rounds,
                "Adaptive budget: deep document"
            );
        }

        let mut state = WorkerState::new(ctx.root(), adaptive_rounds);
        let ls_result = tools::ls(ctx, &state);
        state.set_feedback(ls_result.feedback);

        // --- Phase 1.5: Navigation planning ---
        if state.remaining > 0 && !llm_budget_exhausted!() {
            let plan_prompt = build_plan_prompt(
                &query, task_ref, &state.last_feedback, ctx.doc_name, &preserved_hits, ctx,
            );
            match llm.complete(&plan_prompt.0, &plan_prompt.1).await {
                Ok(plan_output) => {
                    llm_calls += 1;
                    let plan_text = plan_output.trim().to_string();
                    if !plan_text.is_empty() {
                        info!(doc = ctx.doc_name, plan_len = plan_text.len(), "Navigation plan generated");
                        emitter.emit_worker_plan_generated(ctx.doc_name, plan_text.len());
                        state.plan = plan_text;
                        state.plan_generated = true;
                    }
                }
                Err(e) => {
                    warn!(doc = ctx.doc_name, error = %e, "Plan LLM call failed");
                }
            }
        }

        // --- Phase 2: Navigation loop ---
        let use_dispatch_prompt = task_ref.is_some();
        const STUCK_THRESHOLD: u32 = 3;

        loop {
            if state.remaining == 0 {
                info!(doc = ctx.doc_name, "Navigation budget exhausted");
                break;
            }
            if llm_budget_exhausted!() {
                info!(doc = ctx.doc_name, llm_calls, max_llm, "LLM call budget exhausted");
                break;
            }

            // Stuck detection
            if state.rounds_since_evidence >= STUCK_THRESHOLD
                && !state.last_feedback.contains("[Warning:")
            {
                state.last_feedback.push_str(&format!(
                    "\n[Warning: No new evidence collected in {} rounds. \
                     Consider using grep, findtree, or cd .. to explore a different path.]",
                    state.rounds_since_evidence
                ));
                emitter.emit_worker_budget_warning(ctx.doc_name, "stuck", state.max_rounds - state.remaining + 1);
            }

            // Mid-budget checkpoint
            let half_budget = state.max_rounds / 2;
            let rounds_used = state.max_rounds - state.remaining;
            if rounds_used == half_budget && !state.check_called && state.remaining > 1
                && !state.last_feedback.contains("[Hint:")
            {
                state.last_feedback.push_str(
                    "\n[Hint: You've used half your budget. Consider running `check` to evaluate if collected evidence is sufficient.]",
                );
                emitter.emit_worker_budget_warning(ctx.doc_name, "half_budget", rounds_used);
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
                    query: &query, task: task_ref,
                    breadcrumb: &state.path_str(),
                    evidence_summary: &state.evidence_summary(),
                    missing_info: &state.missing_info,
                    last_feedback: &state.last_feedback,
                    remaining: state.remaining,
                    max_rounds: state.max_rounds,
                    history: &state.history_text(),
                    visited_titles: &visited_titles,
                    plan: &state.plan,
                })
            };

            // LLM decision
            let round_start = std::time::Instant::now();
            let llm_output = match llm.complete(&system, &user).await {
                Ok(output) => output,
                Err(e) => {
                    warn!(doc = ctx.doc_name, error = %e, "LLM call failed in nav loop");
                    llm_calls += 1;
                    state.dec_round();
                    state.last_feedback = "LLM error occurred, retrying.".to_string();
                    continue;
                }
            };
            llm_calls += 1;

            // Parse command
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

            let round_num = config.max_rounds - state.remaining + 1;
            let evidence_before = state.evidence.len();
            let is_check = matches!(command, Command::Check);

            // Execute
            let step = execute_command(&command, ctx, &mut state, &query, &llm, &mut llm_calls, &emitter).await;

            if !is_check {
                state.rounds_since_evidence = if state.evidence.len() > evidence_before {
                    0
                } else {
                    state.rounds_since_evidence + 1
                };
            }

            // Dynamic re-planning after insufficient check
            if is_check && !state.missing_info.is_empty() && state.remaining >= 3 && !llm_budget_exhausted!() {
                let missing = state.missing_info.clone();
                let replan = build_replan_prompt(&query, task_ref, &state, ctx);
                match llm.complete(&replan.0, &replan.1).await {
                    Ok(new_plan) => {
                        llm_calls += 1;
                        let plan_text = new_plan.trim().to_string();
                        if !plan_text.is_empty() {
                            info!(doc = ctx.doc_name, plan_len = plan_text.len(), "Re-plan generated");
                            emitter.emit_worker_replan(ctx.doc_name, &missing, plan_text.len());
                            state.plan = plan_text;
                        }
                    }
                    Err(e) => {
                        warn!(doc = ctx.doc_name, error = %e, "Re-plan LLM call failed");
                        state.plan.clear();
                    }
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
                format!("{}...", &state.last_feedback[..120])
            } else {
                state.last_feedback.clone()
            };
            state.push_history(format!("{} → {}", cmd_str, feedback_preview));

            // Check termination
            match step {
                Step::Done => {
                    info!(doc = ctx.doc_name, evidence = state.evidence.len(), "Navigation done");
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
        let mut output = state.into_output_with_budget(llm_calls, budget_exhausted);

        if output.evidence.is_empty() {
            output.answer = format!(
                "I was unable to find relevant information in document '{}' to answer your question.",
                ctx.doc_name
            );
        }

        emitter.emit_worker_done(
            ctx.doc_name, output.evidence.len(),
            output.metrics.rounds_used, output.metrics.llm_calls,
            output.metrics.budget_exhausted, output.metrics.plan_generated,
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

/// Compute adaptive rounds based on document depth.
///
/// Deep documents (depth > 2) get extra rounds, capped at 1.5x base.
fn adaptive_rounds(base_rounds: u32, doc_depth: usize) -> u32 {
    if doc_depth <= 2 {
        return base_rounds;
    }
    let extra = (doc_depth - 2) * 2;
    let capped = base_rounds + extra as u32;
    capped.min((base_rounds as f32 * 1.5).ceil() as u32)
}
