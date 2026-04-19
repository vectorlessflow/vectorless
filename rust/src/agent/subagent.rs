// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! SubAgent loop — document navigation and evidence collection.
//!
//! The SubAgent is a pure-function loop:
//! 1. Fast path: keyword lookup → direct hit?
//! 2. Bird's-eye: ls(root) for initial overview
//! 3. Navigation loop: LLM → parse → execute → repeat (max N rounds)
//! 4. Answer synthesis: LLM generates final answer from evidence
//!
//! Called directly for single-doc scope, or dispatched by the Orchestrator.

use tracing::{debug, info, warn};

use crate::llm::LlmClient;
use crate::utils::bm25::{Bm25Engine, FieldDocument, extract_keywords};

use super::config::QueryComplexity;

use super::command::{Command, parse_command};
use super::config::{Config, DocContext, Evidence, Output, Step};
use super::context::FindHit;
use super::events::EventEmitter;
use super::prompts::{
    NavigationParams, SynthesisParams, answer_synthesis, check_sufficiency,
    parse_sufficiency_response, subagent_dispatch, subagent_navigation,
};
use super::state::State;
use super::tools::subagent as tools;

/// Run the SubAgent loop on a single document.
///
/// - `query`: the user's original question
/// - `task`: sub-task description (None when called directly for single-doc)
/// - `ctx`: read-only access to the document's compile artifacts
/// - `config`: agent configuration
/// - `llm`: LLM client for navigation decisions and synthesis
pub async fn run(
    query: &str,
    task: Option<&str>,
    ctx: &DocContext<'_>,
    config: &Config,
    llm: &LlmClient,
    emitter: &EventEmitter,
) -> crate::error::Result<Output> {
    let is_multi_doc = task.is_some();
    emitter.emit_started(query, is_multi_doc);

    info!(
        doc = ctx.doc_name,
        task = task.unwrap_or("(full query)"),
        max_rounds = config.max_rounds,
        max_llm_calls = config.max_llm_calls,
        "SubAgent starting"
    );

    let mut llm_calls: u32 = 0;
    let max_llm = config.max_llm_calls;

    /// Helper: check if we've hit the LLM call budget.
    macro_rules! llm_budget_exhausted {
        () => {
            max_llm > 0 && llm_calls >= max_llm
        };
    }

    // --- Phase 0: Fast path ---
    // Preserve ReasoningIndex hits from fast_path for planning enrichment.
    let mut preserved_hits: Vec<FindHit> = Vec::new();
    if config.enable_fast_path {
        match fast_path(query, ctx, config, emitter) {
            FastPathResult::Hit(output) => {
                info!(doc = ctx.doc_name, "Fast path hit — skipping navigation");
                emitter.emit_completed(
                    output.evidence.len(),
                    output.metrics.llm_calls,
                    output.metrics.rounds_used,
                    true,  // fast_path_hit
                    false, // budget_exhausted
                    false, // plan_generated
                    0,     // evidence_chars
                );
                return Ok(output);
            }
            FastPathResult::Miss(hits) => {
                if !hits.is_empty() {
                    debug!(
                        doc = ctx.doc_name,
                        hit_count = hits.len(),
                        "Fast path miss — preserving {} keyword hits for planning",
                        hits.len()
                    );
                    preserved_hits = hits;
                } else {
                    debug!(doc = ctx.doc_name, "Fast path miss — no keyword hits");
                }
            }
        }
    }

    // --- Phase 1: Bird's-eye view ---
    debug!(doc = ctx.doc_name, "Phase 1: bird's-eye view (ls root)");

    // Adaptive budget: adjust max_rounds and max_llm_calls based on:
    // 1. Query complexity (heuristic: keywords + word count, zero-cost)
    // 2. Document depth (deeper trees need more rounds)
    let doc_depth = ctx.tree.max_depth();
    let complexity = detect_query_complexity(query);
    let base_rounds = match complexity {
        QueryComplexity::Simple => (config.max_rounds * 6 / 10).max(4), // ~60% of default
        QueryComplexity::Medium => config.max_rounds,                   // default
        QueryComplexity::Complex => (config.max_rounds * 15 / 10).max(10), // ~150% of default
    };
    let base_llm = match complexity {
        QueryComplexity::Simple => (config.max_llm_calls * 6 / 10).max(6),
        QueryComplexity::Medium => config.max_llm_calls,
        QueryComplexity::Complex => (config.max_llm_calls * 14 / 10).max(12),
    };
    let max_llm = base_llm;

    // Then scale for deep documents on top of complexity-adjusted base.
    let adaptive_rounds = if doc_depth <= 2 {
        base_rounds
    } else {
        let extra = (doc_depth - 2) * 2;
        let capped = base_rounds + extra as u32;
        capped.min((base_rounds as f32 * 1.5).ceil() as u32)
    };
    if adaptive_rounds != config.max_rounds || base_llm != config.max_llm_calls {
        info!(
            doc = ctx.doc_name,
            doc_depth,
            complexity = ?complexity,
            configured_rounds = config.max_rounds,
            adaptive_rounds,
            configured_llm = config.max_llm_calls,
            adaptive_llm = max_llm,
            "Adaptive budget: query complexity + document depth"
        );
    }

    let mut state = State::new(ctx.root(), adaptive_rounds);
    let ls_result = tools::ls(ctx, &state);
    state.set_feedback(ls_result.feedback);

    // --- Phase 1.5: Navigation planning ---
    // One LLM call to generate a tentative navigation plan from the bird's-eye view.
    // The plan is non-binding guidance injected into subsequent prompts.
    if state.remaining > 0 && !llm_budget_exhausted!() {
        let plan_prompt = build_plan_prompt(
            query,
            task,
            &state.last_feedback,
            ctx.doc_name,
            &preserved_hits,
            ctx,
        );
        match llm.complete(&plan_prompt.0, &plan_prompt.1).await {
            Ok(plan_output) => {
                llm_calls += 1;
                let plan_text = plan_output.trim().to_string();
                if !plan_text.is_empty() {
                    info!(
                        doc = ctx.doc_name,
                        plan_len = plan_text.len(),
                        "Navigation plan generated"
                    );
                    emitter.emit_plan_generated(ctx.doc_name, plan_text.len());
                    state.plan = plan_text;
                    state.plan_generated = true;
                }
            }
            Err(e) => {
                warn!(doc = ctx.doc_name, error = %e, "Plan LLM call failed — continuing without plan");
            }
        }
    }

    // If this SubAgent was dispatched with a task, use dispatch prompt for first round
    let use_dispatch_prompt = task.is_some();

    // --- Phase 2: Navigation loop ---
    /// Rounds without new evidence before triggering stuck warning.
    const STUCK_THRESHOLD: u32 = 3;

    loop {
        // Navigation budget check
        if state.remaining == 0 {
            info!(doc = ctx.doc_name, "Navigation budget exhausted");
            break;
        }

        // Hard LLM call budget check
        if llm_budget_exhausted!() {
            info!(
                doc = ctx.doc_name,
                llm_calls, max_llm, "LLM call budget exhausted"
            );
            break;
        }

        // Stuck detection: inject warning if no progress
        if state.rounds_since_evidence >= STUCK_THRESHOLD
            && !state.last_feedback.contains("[Warning:")
        {
            let stuck_warning = format!(
                "\n[Warning: No new evidence collected in {} rounds. \
                 Consider using grep, findtree, or cd .. to explore a different path.]",
                state.rounds_since_evidence
            );
            state.last_feedback.push_str(&stuck_warning);
            let round_num = state.max_rounds - state.remaining + 1;
            emitter.emit_budget_warning("stuck", round_num);
        }

        // Mid-budget checkpoint: remind LLM to check if it hasn't yet
        let half_budget = state.max_rounds / 2;
        let rounds_used = state.max_rounds - state.remaining;
        if rounds_used == half_budget
            && !state.check_called
            && state.remaining > 1
            && !state.last_feedback.contains("[Hint:")
        {
            state.last_feedback.push_str(
                "\n[Hint: You've used half your budget. Consider running `check` to evaluate if collected evidence is sufficient.]",
            );
            emitter.emit_budget_warning("half_budget", rounds_used);
        }

        // Build prompt
        let (system, user) = if use_dispatch_prompt && state.remaining == config.max_rounds {
            // First round of dispatched SubAgent — use dispatch prompt
            subagent_dispatch(&super::prompts::SubagentDispatchParams {
                original_query: query,
                task: task.unwrap_or(query),
                doc_name: ctx.doc_name,
                breadcrumb: &state.path_str(),
            })
        } else {
            // Resolve visited node titles for prompt
            let visited_titles = format_visited_titles(&state, ctx);
            subagent_navigation(&NavigationParams {
                query,
                task,
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

        // Parse command — detect parse failures (command confidence)
        let command = parse_command(&llm_output);
        let llm_trimmed = llm_output.trim();
        let is_parse_failure = matches!(command, Command::Ls)
            && !llm_trimmed.starts_with("ls")
            && !llm_trimmed.is_empty();

        if is_parse_failure {
            // Preserve LLM's raw output as feedback — it may contain reasoning
            debug!(doc = ctx.doc_name, raw = %llm_trimmed, "Parse failure — preserving raw output");
            let raw_preview = if llm_trimmed.len() > 200 {
                format!("{}...", &llm_trimmed[..200])
            } else {
                llm_trimmed.to_string()
            };
            state.last_feedback = format!(
                "Your output was not recognized as a valid command:\n\"{}\"\n\n\
                 Please output exactly one command (ls, cd, cat, head, find, findtree, grep, wc, pwd, check, or done).",
                raw_preview
            );
            // Don't consume a navigation round for parse failures (but LLM call already counted above)
            state.push_history(format!("(unrecognized) → parse failure"));
            continue;
        }

        debug!(doc = ctx.doc_name, ?command, "Parsed command");

        let round_num = config.max_rounds - state.remaining + 1;
        let evidence_before = state.evidence.len();
        let is_check = matches!(command, Command::Check);

        // Execute command
        let step = execute_command(
            &command,
            ctx,
            &mut state,
            query,
            llm,
            &mut llm_calls,
            emitter,
        )
        .await;

        // Only consume navigation budget for non-check commands
        // (check is a verification action, not navigation — it shouldn't compete for nav budget)
        if !is_check {
            state.rounds_since_evidence = if state.evidence.len() > evidence_before {
                0
            } else {
                state.rounds_since_evidence + 1
            };
        }

        // Dynamic re-planning: when check returned INSUFFICIENT and budget allows,
        // generate a focused new plan to guide remaining navigation.
        if is_check
            && !state.missing_info.is_empty()
            && state.remaining >= 3
            && !llm_budget_exhausted!()
        {
            let missing = state.missing_info.clone();
            let replan = build_replan_prompt(query, task, &state, ctx);
            match llm.complete(&replan.0, &replan.1).await {
                Ok(new_plan) => {
                    llm_calls += 1;
                    let plan_text = new_plan.trim().to_string();
                    if !plan_text.is_empty() {
                        info!(
                            doc = ctx.doc_name,
                            plan_len = plan_text.len(),
                            "Re-plan generated after insufficient evidence"
                        );
                        emitter.emit_replan_generated(ctx.doc_name, &missing, plan_text.len());
                        state.plan = plan_text;
                    }
                }
                Err(e) => {
                    warn!(doc = ctx.doc_name, error = %e, "Re-plan LLM call failed");
                    // Fall back to ReAct free exploration
                    state.plan.clear();
                }
            }
            // Clear missing_info so we don't re-plan again next round
            state.missing_info.clear();
        } else if is_check && !state.missing_info.is_empty() {
            // Budget too tight for re-planning — clear plan for ReAct free exploration
            state.plan.clear();
            state.missing_info.clear();
        }

        // Emit round event
        let cmd_str = format!("{:?}", command);
        let success = !matches!(step, Step::ForceDone(_));
        let round_elapsed = round_start.elapsed().as_millis() as u64;
        emitter.emit_round(round_num, &cmd_str, success, round_elapsed);

        // Push to ReAct history
        let feedback_preview = if state.last_feedback.len() > 120 {
            format!("{}...", &state.last_feedback[..120])
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
                // Only consume navigation budget for non-check commands.
                // check is verification, not exploration — it shouldn't compete
                // with ls/cd/cat for the navigation budget.
                if !is_check {
                    state.dec_round();
                }
            }
        }
    }

    let budget_exhausted = state.remaining == 0 || llm_budget_exhausted!();

    // --- Phase 3: Answer synthesis ---
    let missing_info = state.missing_info.clone();
    let mut output = state.into_output_with_budget(llm_calls, budget_exhausted);

    if config.enable_synthesis && !output.evidence.is_empty() {
        debug!(
            doc = ctx.doc_name,
            evidence = output.evidence.len(),
            "Phase 3: synthesizing answer from evidence"
        );
        let evidence_text = format_evidence_for_synthesis(&output.evidence);
        let (system, user) = answer_synthesis(&SynthesisParams {
            query,
            evidence_text: &evidence_text,
            missing_info: &missing_info,
        });

        match llm.complete(&system, &user).await {
            Ok(answer) => {
                output.answer = answer.trim().to_string();
                output.metrics.llm_calls += 1;
                info!(
                    doc = ctx.doc_name,
                    answer_len = output.answer.len(),
                    "Synthesis complete"
                );
                emitter.emit_synthesis(output.answer.len());
            }
            Err(e) => {
                warn!(doc = ctx.doc_name, error = %e, "Synthesis LLM call failed — using raw evidence");
                output.answer = format_evidence_as_answer(&output.evidence);
            }
        }
    } else if !output.evidence.is_empty() {
        debug!(
            doc = ctx.doc_name,
            "Synthesis disabled — concatenating raw evidence"
        );
        output.answer = format_evidence_as_answer(&output.evidence);
    } else {
        info!(
            doc = ctx.doc_name,
            "No evidence collected — returning not-found message"
        );
        output.answer = format!(
            "I was unable to find relevant information in document '{}' to answer your question.",
            ctx.doc_name
        );
    }

    emitter.emit_completed(
        output.evidence.len(),
        output.metrics.llm_calls,
        output.metrics.rounds_used,
        output.metrics.fast_path_hit,
        output.metrics.budget_exhausted,
        output.metrics.plan_generated,
        output.metrics.evidence_chars,
    );

    info!(
        doc = ctx.doc_name,
        evidence = output.evidence.len(),
        rounds = output.metrics.rounds_used,
        llm_calls = output.metrics.llm_calls,
        "SubAgent complete"
    );

    Ok(output)
}

/// Result of the fast-path attempt.
///
/// On hit: returns the output directly.
/// On miss: returns the keyword hits from ReasoningIndex so the planning phase can use them.
enum FastPathResult {
    /// Fast path hit — high-confidence direct answer.
    Hit(Output),
    /// Fast path miss, but ReasoningIndex returned keyword hits.
    /// These hits are valuable context for Phase 1.5 planning.
    Miss(Vec<FindHit>),
}

/// Try the fast path: extract keywords → look up in ReasoningIndex → return if confident.
///
/// When the best hit is below threshold, returns `Miss` with the hits so they can
/// be injected into the planning prompt — avoiding a redundant index lookup.
fn fast_path(
    query: &str,
    ctx: &DocContext<'_>,
    config: &Config,
    emitter: &EventEmitter,
) -> FastPathResult {
    let keywords = extract_keywords(query);
    if keywords.is_empty() {
        return FastPathResult::Miss(Vec::new());
    }

    let hits: Vec<FindHit> = ctx.find_all(&keywords);
    if hits.is_empty() {
        return FastPathResult::Miss(Vec::new());
    }

    // Find the best matching node
    let best_entry = hits
        .iter()
        .flat_map(|hit| hit.entries.iter().map(|e| (hit.keyword.clone(), e)))
        .max_by(|a, b| {
            a.1.weight
                .partial_cmp(&b.1.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

    let Some((best_kw, best)) = best_entry else {
        return FastPathResult::Miss(hits);
    };

    if best.weight < config.fast_path_threshold {
        debug!(
            keyword = %best_kw,
            weight = best.weight,
            threshold = config.fast_path_threshold,
            "Fast path: best hit below threshold — passing hits to planning"
        );
        return FastPathResult::Miss(hits);
    }

    // Read content from the best node
    let content = ctx.cat(best.node_id).unwrap_or("").to_string();
    let title = ctx
        .node_title(best.node_id)
        .unwrap_or("unknown")
        .to_string();

    if content.is_empty() {
        return FastPathResult::Miss(hits);
    }

    info!(
        keyword = %best_kw,
        node = %title,
        weight = best.weight,
        "Fast path hit"
    );

    emitter.emit_fast_path(&best_kw, &title, best.weight);

    FastPathResult::Hit(Output::fast_path(
        content.clone(),
        vec![Evidence {
            source_path: title.clone(),
            node_title: title,
            content,
            doc_name: Some(ctx.doc_name.to_string()),
        }],
    ))
}

/// Execute a single parsed command, mutating state.
///
/// Returns a `Step` indicating whether to continue or stop.
async fn execute_command(
    command: &Command,
    ctx: &DocContext<'_>,
    state: &mut State,
    query: &str,
    llm: &LlmClient,
    llm_calls: &mut u32,
    emitter: &EventEmitter,
) -> Step {
    match command {
        Command::Ls => {
            let result = tools::ls(ctx, state);
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::Cd { target } => {
            let result = tools::cd(target, ctx, state);
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::CdUp => {
            let result = tools::cd_up(ctx, state);
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::Cat { target } => {
            let evidence_before = state.evidence.len();
            let result = tools::cat(target, ctx, state);
            state.set_feedback(result.feedback);
            // Emit evidence event if new evidence was added
            if state.evidence.len() > evidence_before {
                if let Some(ev) = state.evidence.last() {
                    info!(
                        doc = ctx.doc_name,
                        node = %ev.node_title,
                        path = %ev.source_path,
                        len = ev.content.len(),
                        total = state.evidence.len(),
                        "Evidence collected"
                    );
                    emitter.emit_evidence(
                        &ev.node_title,
                        &ev.source_path,
                        ev.content.len(),
                        state.evidence.len(),
                    );
                }
            }
            Step::Continue
        }

        Command::Find { keyword } => {
            let feedback = match ctx.find(keyword) {
                Some(hit) => {
                    // Sort by weight descending, dedup by node_id (keep highest weight)
                    let mut entries = hit.entries.clone();
                    entries.sort_by(|a, b| {
                        b.weight
                            .partial_cmp(&a.weight)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    let mut seen_nodes = std::collections::HashSet::new();
                    let mut output = format!("Results for '{}':\n", keyword);
                    for entry in &entries {
                        if !seen_nodes.insert(entry.node_id) {
                            continue; // skip duplicate node
                        }
                        let title = ctx.node_title(entry.node_id).unwrap_or("unknown");
                        let summary = ctx
                            .nav_entry(entry.node_id)
                            .map(|e| e.overview.as_str())
                            .unwrap_or("");
                        output.push_str(&format!(
                            "  - {} (depth {}, weight {:.2})",
                            title, entry.depth, entry.weight
                        ));
                        if !summary.is_empty() {
                            output.push_str(&format!(" — {}", summary));
                        }
                        output.push('\n');
                    }
                    output
                }
                None => format!("No results for '{}'", keyword),
            };
            state.set_feedback(feedback);
            Step::Continue
        }

        Command::Pwd => {
            let result = tools::pwd(state);
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::Check => {
            let evidence_summary = state.evidence_summary();

            // Heuristic pre-check: skip LLM call when evidence is obviously sufficient.
            // Uses content length + quality indicators (from legacy ThresholdChecker).
            let all_content: String = state.evidence.iter().map(|e| e.content.as_str()).collect();
            let heuristic = heuristic_sufficiency(&all_content);
            if heuristic.is_sufficient() && !all_content.is_empty() {
                info!(
                    doc = ctx.doc_name,
                    evidence = state.evidence.len(),
                    content_len = all_content.len(),
                    quality = heuristic.quality_score,
                    "Heuristic pre-check: sufficient (skipping LLM call)"
                );
                state.check_called = true;
                state.check_count += 1;
                emitter.emit_sufficiency(true, state.evidence.len());
                state.last_feedback = "Evidence is sufficient. Use done to finish.".to_string();
                return Step::Done;
            }

            // Fall through to LLM-based check
            let (system, user) = check_sufficiency(query, &evidence_summary);

            match llm.complete(&system, &user).await {
                Ok(response) => {
                    *llm_calls += 1;
                    state.check_called = true;
                    state.check_count += 1;
                    let sufficient = parse_sufficiency_response(&response);
                    info!(
                        doc = ctx.doc_name,
                        sufficient,
                        evidence = state.evidence.len(),
                        "Sufficiency check"
                    );
                    emitter.emit_sufficiency(sufficient, state.evidence.len());
                    if sufficient {
                        state.last_feedback =
                            "Evidence is sufficient. Use done to finish.".to_string();
                        Step::Done
                    } else {
                        // Extract what's missing from the LLM response
                        let reason = response
                            .trim()
                            .strip_prefix("INSUFFICIENT")
                            .unwrap_or(response.trim())
                            .trim()
                            .trim_start_matches(|c: char| c == '-' || c == ' ');
                        if !reason.is_empty() {
                            state.missing_info = reason.to_string();
                        }
                        state.set_feedback(format!(
                            "Evidence not yet sufficient: {}",
                            response.trim()
                        ));
                        Step::Continue
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Check LLM call failed");
                    state.last_feedback = "Could not evaluate sufficiency.".to_string();
                    Step::Continue
                }
            }
        }

        Command::Done => {
            state.last_feedback = "Navigation complete.".to_string();
            Step::Done
        }

        Command::Grep { pattern } => {
            let result = tools::grep(pattern, ctx, state);
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::Head { target, lines } => {
            let result = tools::head(target, *lines, ctx, state);
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::FindTree { pattern } => {
            let result = tools::find_tree(pattern, ctx);
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::Wc { target } => {
            let result = tools::wc(target, ctx, state);
            state.set_feedback(result.feedback);
            Step::Continue
        }
    }
}

/// Maximum total chars for keyword + semantic sections in planning prompt.
const PLAN_CONTEXT_BUDGET: usize = 1500;

/// Build the navigation planning prompt (Phase 1.5).
///
/// One-shot LLM call after bird's-eye view to generate a tentative navigation plan.
/// Enriched with:
/// - Keyword hits from the ReasoningIndex (preserved from fast-path miss)
/// - Ancestor paths showing where each hit sits in the document tree
/// - Semantic hints from question_hints and topic_tags matching
fn build_plan_prompt(
    query: &str,
    task: Option<&str>,
    ls_output: &str,
    doc_name: &str,
    keyword_hits: &[FindHit],
    ctx: &DocContext<'_>,
) -> (String, String) {
    let task_section = match task {
        Some(t) => format!("\nYour specific task: {}", t),
        None => String::new(),
    };

    let query_keywords = extract_keywords(query);
    let query_lower = query.to_lowercase();

    // --- Keyword hits with ancestor path expansion ---
    let mut keyword_section = if keyword_hits.is_empty() {
        String::new()
    } else {
        let mut section =
            String::from("\nKeyword index matches (use these to prioritize navigation):\n");
        for hit in keyword_hits {
            let mut entries = hit.entries.clone();
            entries.sort_by(|a, b| {
                b.weight
                    .partial_cmp(&a.weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            // Dedup by node_id, keep highest weight
            let mut seen = std::collections::HashSet::new();
            for entry in &entries {
                if !seen.insert(entry.node_id) {
                    continue;
                }
                let ancestor_path = build_ancestor_path(entry.node_id, ctx);
                section.push_str(&format!(
                    "  - keyword '{}' → {} (depth {}, weight {:.2})\n",
                    hit.keyword, ancestor_path, entry.depth, entry.weight
                ));
                // Budget check
                if section.len() > PLAN_CONTEXT_BUDGET {
                    section.push_str("  ... (more hits truncated)\n");
                    break;
                }
            }
            if section.len() > PLAN_CONTEXT_BUDGET {
                break;
            }
        }
        section
    };

    // --- Multi-level expansion: for deep keyword hits, show siblings at the target level ---
    let deep_expansion = build_deep_expansion(keyword_hits, ctx);
    if !deep_expansion.is_empty() {
        if keyword_section.len() + deep_expansion.len() <= PLAN_CONTEXT_BUDGET {
            keyword_section.push_str(&deep_expansion);
        }
    }

    // --- Semantic hints: match query against question_hints and topic_tags ---
    let semantic_section = build_semantic_hints(&query_keywords, &query_lower, ctx);

    let system = "You are a document navigation planner. Given a user question, the top-level \
         document structure, keyword index matches, and semantic hints, output a brief navigation \
         plan: which sections to visit and in what order. Prioritize sections that matched keywords \
         or semantic hints. The plan should be 2-5 steps. Each step should be a specific action \
         like \"cd to X, then cat Y\" or \"grep for Z in subtree\". \
         Pay attention to 'Can answer' and 'Topics' annotations in the structure listing — \
         they indicate what questions each section addresses. \
         Output only the plan, nothing else.\n\n\
         Example plan for \"What is the Q1 revenue?\":\n\
         1. cd to Revenue (matched keyword 'revenue')\n\
         2. ls to see sub-sections\n\
         3. cat Q1 Report\n\
         4. check\n\
         5. done".to_string();

    let user = format!(
        "Document: {doc_name}\n\
         Top-level structure:\n{ls_output}{keyword_section}{semantic_section}\
         User question: {query}{task_section}\n\n\
         Navigation plan:"
    );

    (system, user)
}

/// Build the ancestor path string for a node (e.g., "root > Chapter 1 > Section 1.2").
fn build_ancestor_path(node_id: crate::document::NodeId, ctx: &DocContext<'_>) -> String {
    // ancestors_iter returns [node, parent, ..., root], so reverse to get root-to-node order.
    let mut path: Vec<crate::document::NodeId> = ctx.tree.ancestors_iter(node_id).collect();
    path.reverse();
    path.iter()
        .filter_map(|&id| ctx.node_title(id))
        .collect::<Vec<_>>()
        .join(" > ")
}

/// Build semantic hints section using BM25 scoring over child routes.
///
/// Instead of binary keyword matching, this uses a lightweight `Bm25Engine` to
/// score each root-level child route against the query. The BM25 engine receives
/// each route's title, description, overview, question_hints, and topic_tags as
/// fields with different weights — title matches rank highest.
///
/// Routes with non-zero BM25 scores are injected into the planning prompt with
/// their score and any matching question/topic annotations, giving the planner
/// continuous relevance signals instead of binary match/no-match.
fn build_semantic_hints(
    query_keywords: &[String],
    query_lower: &str,
    ctx: &DocContext<'_>,
) -> String {
    let root = ctx.root();
    let routes = match ctx.ls(root) {
        Some(r) => r,
        None => return String::new(),
    };

    if routes.is_empty() {
        return String::new();
    }

    // --- BM25 scoring over child routes ---
    // Build a FieldDocument for each route: title, description, overview+hints+tags.
    let field_docs: Vec<FieldDocument<String>> = routes
        .iter()
        .map(|route| {
            let nav = ctx.nav_entry(route.node_id);
            let overview = nav.map(|n| n.overview.as_str()).unwrap_or("");
            let hints_text = nav.map(|n| n.question_hints.join(" ")).unwrap_or_default();
            let tags_text = nav.map(|n| n.topic_tags.join(" ")).unwrap_or_default();

            // Content field combines all metadata for rich matching.
            let content = if overview.is_empty() && hints_text.is_empty() && tags_text.is_empty() {
                String::new()
            } else {
                format!("{} {} {}", overview, hints_text, tags_text)
            };

            FieldDocument::new(
                route.title.clone(),
                route.title.clone(),
                route.description.clone(),
                content,
            )
        })
        .collect();

    let engine = Bm25Engine::fit_to_corpus(&field_docs);
    let bm25_results: std::collections::HashMap<String, f32> = engine
        .search_weighted(query_lower, routes.len())
        .into_iter()
        .collect();

    // --- Also do keyword-level matching for annotation ---
    let mut section = String::new();
    let budget_remaining = PLAN_CONTEXT_BUDGET.saturating_sub(section.len());

    for route in routes {
        let nav = match ctx.nav_entry(route.node_id) {
            Some(n) => n,
            None => continue,
        };

        let bm25_score = bm25_results.get(&route.title).copied().unwrap_or(0.0);

        // Skip routes with zero BM25 score (no relevance signal at all)
        if bm25_score <= 0.0 {
            continue;
        }

        let mut annotations = Vec::new();

        // Annotate with keyword matches for explainability
        for hint in &nav.question_hints {
            let hint_lower = hint.to_lowercase();
            for kw in query_keywords {
                if hint_lower.contains(&kw.to_lowercase()) {
                    annotations.push(format!("question \"{}\"", hint));
                    break;
                }
            }
            if !annotations.iter().any(|a| a.contains(&hint.clone())) {
                for word in hint_lower.split_whitespace() {
                    if word.len() > 3 && query_lower.contains(word) {
                        annotations.push(format!("question \"{}\"", hint));
                        break;
                    }
                }
            }
        }

        for tag in &nav.topic_tags {
            let tag_lower = tag.to_lowercase();
            for kw in query_keywords {
                if tag_lower.contains(&kw.to_lowercase()) || kw.to_lowercase().contains(&tag_lower)
                {
                    annotations.push(format!("topic \"{}\"", tag));
                    break;
                }
            }
            if !annotations
                .iter()
                .any(|a| a.contains(&format!("topic \"{}\"", tag)))
            {
                if query_lower.contains(&tag_lower) && tag.len() > 2 {
                    annotations.push(format!("topic \"{}\"", tag));
                }
            }
        }

        let annotation_str = if annotations.is_empty() {
            String::new()
        } else {
            format!(", {}", annotations.join(", "))
        };

        let line = format!(
            "  - Section '{}' — BM25: {:.2}{}\n",
            route.title, bm25_score, annotation_str
        );
        if section.len() + line.len() > budget_remaining {
            break;
        }
        section.push_str(&line);
    }

    if section.is_empty() {
        String::new()
    } else {
        format!(
            "\nSemantic hints (BM25-scored sections, higher = more relevant):\n{}",
            section
        )
    }
}

/// For keyword hits that land in deep nodes (depth >= 2), expand the parent node's children
/// so the planner sees the target level's full context — not just the root-level structure.
fn build_deep_expansion(keyword_hits: &[FindHit], ctx: &DocContext<'_>) -> String {
    if keyword_hits.is_empty() {
        return String::new();
    }

    // Collect unique parent nodes of deep hits (depth >= 2)
    let mut seen_parents = std::collections::HashSet::new();
    let mut expansion = String::new();

    for hit in keyword_hits {
        for entry in &hit.entries {
            if entry.depth < 2 {
                continue;
            }
            // Get parent of the hit node
            let parent = match ctx.parent(entry.node_id) {
                Some(p) => p,
                None => continue,
            };
            if !seen_parents.insert(parent) {
                continue;
            }
            let routes = match ctx.ls(parent) {
                Some(r) => r,
                None => continue,
            };
            let parent_title = ctx.node_title(parent).unwrap_or("unknown");
            expansion.push_str(&format!(
                "Siblings near keyword hit '{}' (under {}):\n",
                hit.keyword, parent_title
            ));
            for route in routes {
                let marker = if ctx.node_title(entry.node_id) == Some(&route.title) {
                    " ← keyword hit"
                } else {
                    ""
                };
                expansion.push_str(&format!(
                    "  - {} ({} leaves){}\n",
                    route.title, route.leaf_count, marker
                ));
            }
            expansion.push('\n');
            // Cap expansion at 500 chars
            if expansion.len() > 500 {
                expansion.push_str("  ... (more expansions truncated)\n");
                break;
            }
        }
        if expansion.len() > 500 {
            break;
        }
    }

    expansion
}

/// Build unvisited sibling branch hints for structured backtracking.
///
/// Shows:
/// - Unvisited siblings of the current node (same-level alternatives)
/// - Unvisited siblings of the parent node (if current branch seems exhausted)
fn build_sibling_hints(state: &State, ctx: &DocContext<'_>) -> String {
    let mut hints = String::new();

    // 1. Unvisited siblings of current node
    if let Some(parent) = ctx.parent(state.current_node) {
        if let Some(routes) = ctx.ls(parent) {
            let unvisited: Vec<&crate::document::ChildRoute> = routes
                .iter()
                .filter(|r| !state.visited.contains(&r.node_id))
                .collect();
            if !unvisited.is_empty() {
                hints.push_str("Unvisited sibling branches at current level:\n");
                for route in &unvisited {
                    hints.push_str(&format!(
                        "  - {} ({} leaves)\n",
                        route.title, route.leaf_count
                    ));
                }
            }
        }

        // 2. Also show parent-level siblings (aunt/uncle nodes) if not at root
        if let Some(grandparent) = ctx.parent(parent) {
            if let Some(routes) = ctx.ls(grandparent) {
                let unvisited_parent_siblings: Vec<&crate::document::ChildRoute> = routes
                    .iter()
                    .filter(|r| !state.visited.contains(&r.node_id) && r.node_id != parent)
                    .collect();
                if !unvisited_parent_siblings.is_empty() {
                    hints.push_str("Unvisited branches at parent level (cd .. then explore):\n");
                    for route in &unvisited_parent_siblings {
                        hints.push_str(&format!(
                            "  - {} ({} leaves)\n",
                            route.title, route.leaf_count
                        ));
                    }
                }
            }
        }
    }

    if hints.is_empty() {
        String::new()
    } else {
        format!("\n{}", hints)
    }
}

/// Build a focused re-planning prompt when check returns INSUFFICIENT.
///
/// Unlike the initial planning prompt (Phase 1.5) which starts from root-level structure,
/// this uses the current navigation state: position, visited nodes, collected evidence,
/// and what's specifically missing.
fn build_replan_prompt(
    query: &str,
    task: Option<&str>,
    state: &State,
    ctx: &DocContext<'_>,
) -> (String, String) {
    let task_section = match task {
        Some(t) => format!("\nOriginal sub-task: {}", t),
        None => String::new(),
    };

    let visited = format_visited_titles(state, ctx);
    let evidence_summary = state.evidence_summary();

    // Show current position's children for local navigation context
    let current_children = match ctx.ls(state.current_node) {
        Some(routes) if !routes.is_empty() => {
            let items: Vec<String> = routes
                .iter()
                .map(|r| format!("  - {} ({} leaves)", r.title, r.leaf_count))
                .collect();
            format!("Children at current position:\n{}\n", items.join("\n"))
        }
        _ => "Current position is a leaf node — consider cd .. to go back.\n".to_string(),
    };

    // Show unvisited sibling branches for structured backtracking
    let sibling_hints = build_sibling_hints(state, ctx);

    let system = "You are re-planning a document navigation strategy. The previous plan did not \
         find sufficient evidence. Given what's been found and what's still missing, generate a \
         focused 2-3 step plan. Each step should be a specific action like \
         \"cd to X, then cat Y\" or \"grep for Z in current subtree\". \
         Prefer exploring unvisited branches. If current branch is exhausted, cd .. and try \
         a different path. Output only the plan, nothing else."
        .to_string();

    let user = format!(
        "Original question: {query}{task_section}\n\
         Current position: /{}\n\
         Evidence collected so far:\n{evidence_summary}\n\
         What's missing: {}\n\
         Already visited: {visited}\n\
         {current_children}\
         {sibling_hints}\
         Remaining rounds: {}/{}\n\n\
         Revised navigation plan:",
        state.path_str(),
        state.missing_info,
        state.remaining,
        state.max_rounds,
    );

    (system, user)
}

/// Detect query complexity using heuristics (zero-cost, no LLM call).
///
/// Extracted from the legacy ComplexityDetector — pure function with
/// no dependencies. Used to adapt navigation budget before entering the loop.
fn detect_query_complexity(query: &str) -> QueryComplexity {
    let query_lower = query.to_lowercase();
    let word_count = estimate_word_count(query);

    // Complex indicators (English + Chinese)
    let complex_indicators = [
        "compare",
        "contrast",
        "analyze",
        "evaluate",
        "synthesize",
        "explain why",
        "how does",
        "relationship between",
        "cause and effect",
        "对比",
        "分析",
        "评估",
        "综合",
        "为什么",
        "原因",
        "关系",
        "影响",
        "区别",
        "异同",
    ];
    for indicator in &complex_indicators {
        if query_lower.contains(indicator) {
            return QueryComplexity::Complex;
        }
    }

    // Simple indicators
    let simple_indicators = [
        "what is",
        "define",
        "list",
        "who",
        "when",
        "where",
        "什么是",
        "定义",
        "列表",
        "谁",
        "何时",
        "哪里",
        "在哪",
    ];
    for indicator in &simple_indicators {
        if query_lower.contains(indicator) && word_count <= 15 {
            return QueryComplexity::Simple;
        }
    }

    // Multiple questions → complex
    let question_marks = query.matches('?').count() + query.matches('？').count();
    if question_marks > 1 {
        return QueryComplexity::Complex;
    }

    // Word count classification
    if word_count <= 5 {
        QueryComplexity::Simple
    } else if word_count <= 15 {
        QueryComplexity::Medium
    } else {
        QueryComplexity::Complex
    }
}

/// Estimate word count, handling both CJK and Latin text.
fn estimate_word_count(text: &str) -> usize {
    let mut count = 0usize;
    let mut in_latin_word = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if in_latin_word {
                count += 1;
                in_latin_word = false;
            }
        } else if ch.is_ascii_alphanumeric() {
            in_latin_word = true;
        } else if is_cjk_char(ch) {
            if in_latin_word {
                count += 1;
                in_latin_word = false;
            }
            count += 1;
        } else if in_latin_word {
            count += 1;
            in_latin_word = false;
        }
    }
    if in_latin_word {
        count += 1;
    }
    count
}

/// Check if a character is CJK (Chinese/Japanese/Korean).
fn is_cjk_char(ch: char) -> bool {
    let cp = ch as u32;
    (0x4E00..=0x9FFF).contains(&cp)
        || (0x3400..=0x4DBF).contains(&cp)
        || (0x20000..=0x2A6DF).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0x3000..=0x303F).contains(&cp)
        || (0x3040..=0x309F).contains(&cp)
        || (0x30A0..=0x30FF).contains(&cp)
}

/// Result of the heuristic sufficiency pre-check.
struct SufficiencyHint {
    /// Estimated token count (~4 chars per token).
    estimated_tokens: usize,
    /// Content quality score (0.0 - 1.0).
    quality_score: f32,
}

impl SufficiencyHint {
    /// Whether the heuristic considers evidence sufficient.
    /// Requires both enough content AND reasonable quality.
    fn is_sufficient(&self) -> bool {
        self.estimated_tokens >= 500 && self.quality_score > 0.5
    }
}

/// Heuristic sufficiency check — extracted from legacy ThresholdChecker.
///
/// Zero-cost check that can skip an LLM call when evidence is obviously sufficient.
/// Uses content length and quality indicators (sentence structure, vocabulary diversity).
fn heuristic_sufficiency(content: &str) -> SufficiencyHint {
    let estimated_tokens = content.len() / 4;
    let mut score = 0.0f32;

    // Sentence endings (periods, question marks, exclamation marks)
    let sentence_endings = content.matches('.').count()
        + content.matches('?').count()
        + content.matches('!').count()
        + content.matches('。').count()
        + content.matches('？').count()
        + content.matches('！').count();
    score += (sentence_endings as f32 * 0.05).min(0.3);

    // Paragraph breaks
    let paragraphs = content.matches("\n\n").count();
    score += (paragraphs as f32 * 0.1).min(0.3);

    // Structure markers
    if content.contains(':') || content.contains('-') || content.contains('：') {
        score += 0.1;
    }

    // Vocabulary diversity (penalize repetitive content)
    let words: Vec<&str> = content.split_whitespace().collect();
    if words.len() > 10 {
        let unique_ratio = words.iter().collect::<std::collections::HashSet<_>>().len() as f32
            / words.len() as f32;
        score += unique_ratio * 0.3;
    }

    SufficiencyHint {
        estimated_tokens,
        quality_score: score.min(1.0),
    }
}

/// Resolve visited NodeIds to their titles for prompt injection.
fn format_visited_titles(state: &State, ctx: &DocContext<'_>) -> String {
    if state.visited.is_empty() {
        return "(none)".to_string();
    }
    state
        .visited
        .iter()
        .filter_map(|&node_id| ctx.node_title(node_id).map(|t| t.to_string()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Maximum total characters for evidence in the synthesis prompt.
/// Prevents runaway token costs when many evidence items are collected.
const SYNTHESIS_EVIDENCE_CAP: usize = 8000;

/// Format evidence items for the synthesis prompt, with a total character cap.
///
/// Each item is included in full until the cap is reached. Items that would
/// exceed the cap are truncated with a "[truncated]" marker.
fn format_evidence_for_synthesis(evidence: &[Evidence]) -> String {
    let mut result = String::new();
    for e in evidence {
        let item = format!(
            "[{}] (source: {})\n{}",
            e.node_title, e.source_path, e.content
        );
        if result.len() + item.len() + 2 > SYNTHESIS_EVIDENCE_CAP {
            // Truncate this item to fit the remaining budget
            let remaining = SYNTHESIS_EVIDENCE_CAP.saturating_sub(result.len());
            if remaining > 50 {
                result.push_str(&format!(
                    "[{}] (source: {})\n{}...[truncated]\n",
                    e.node_title,
                    e.source_path,
                    &e.content[..remaining.min(e.content.len())]
                ));
            }
            result.push_str(&format!(
                "\n... and {} more evidence items truncated to fit budget.\n",
                evidence.len()
                    - evidence
                        .iter()
                        .position(|x| x.node_title == e.node_title)
                        .unwrap_or(0)
                    - 1
            ));
            break;
        }
        result.push_str(&item);
        result.push_str("\n\n");
    }
    result
}

/// Format evidence as a simple answer (fallback when synthesis is disabled or fails).
fn format_evidence_as_answer(evidence: &[Evidence]) -> String {
    evidence
        .iter()
        .map(|e| {
            format!(
                "**{}** (at {}):\n{}",
                e.node_title, e.source_path, e.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_evidence_for_synthesis() {
        let evidence = vec![Evidence {
            source_path: "root/A".to_string(),
            node_title: "A".to_string(),
            content: "content of A".to_string(),
            doc_name: None,
        }];
        let formatted = format_evidence_for_synthesis(&evidence);
        assert!(formatted.contains("[A]"));
        assert!(formatted.contains("content of A"));
    }

    #[test]
    fn test_format_evidence_as_answer() {
        let evidence = vec![Evidence {
            source_path: "root/B".to_string(),
            node_title: "B".to_string(),
            content: "content of B".to_string(),
            doc_name: None,
        }];
        let formatted = format_evidence_as_answer(&evidence);
        assert!(formatted.contains("**B**"));
        assert!(formatted.contains("content of B"));
    }

    #[test]
    fn test_fast_path_no_keywords() {
        let tree = crate::document::DocumentTree::new("Root", "content");
        let nav = crate::document::NavigationIndex::new();
        let ridx = crate::document::ReasoningIndex::default();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &ridx,
            doc_name: "test",
        };
        let config = Config::default();
        let emitter = EventEmitter::noop();

        // Query with only stopwords won't extract keywords
        let result = fast_path("the a an", &ctx, &config, &emitter);
        assert!(matches!(result, FastPathResult::Miss(ref hits) if hits.is_empty()));
    }

    #[test]
    fn test_fast_path_empty_index() {
        let tree = crate::document::DocumentTree::new("Root", "content");
        let nav = crate::document::NavigationIndex::new();
        let ridx = crate::document::ReasoningIndex::default();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &ridx,
            doc_name: "test",
        };
        let config = Config::default();
        let emitter = EventEmitter::noop();

        let result = fast_path("revenue finance", &ctx, &config, &emitter);
        assert!(matches!(result, FastPathResult::Miss(ref hits) if hits.is_empty()));
    }

    // --- Tests for new features ---

    /// Helper to build a tree with NavEntry metadata (question_hints, topic_tags).
    fn build_semantic_test_tree() -> (
        crate::document::DocumentTree,
        crate::document::NavigationIndex,
        crate::document::NodeId, // root
        crate::document::NodeId, // revenue child
        crate::document::NodeId, // expenses child
    ) {
        use crate::document::{ChildRoute, NavEntry};

        let mut tree = crate::document::DocumentTree::new("Root", "root content");
        let root = tree.root();
        let revenue = tree.add_child(root, "Revenue", "revenue content");
        let expenses = tree.add_child(root, "Expenses", "expense content");

        let mut nav = crate::document::NavigationIndex::new();

        // Root entry
        nav.add_entry(
            root,
            NavEntry {
                overview: "Annual financial report".to_string(),
                question_hints: vec!["What is the financial overview?".to_string()],
                topic_tags: vec!["finance".to_string()],
                leaf_count: 4,
                level: 0,
            },
        );

        // Revenue entry with question_hints and topic_tags
        nav.add_child_routes(
            root,
            vec![
                ChildRoute {
                    node_id: revenue,
                    title: "Revenue".to_string(),
                    description: "Revenue breakdown".to_string(),
                    leaf_count: 2,
                },
                ChildRoute {
                    node_id: expenses,
                    title: "Expenses".to_string(),
                    description: "Cost analysis".to_string(),
                    leaf_count: 2,
                },
            ],
        );
        nav.add_entry(
            revenue,
            NavEntry {
                overview: "Revenue figures for 2024".to_string(),
                question_hints: vec![
                    "What is the total revenue?".to_string(),
                    "What was the Q1 revenue?".to_string(),
                ],
                topic_tags: vec![
                    "revenue".to_string(),
                    "sales".to_string(),
                    "income".to_string(),
                ],
                leaf_count: 2,
                level: 1,
            },
        );
        nav.add_entry(
            expenses,
            NavEntry {
                overview: "Operating expenses".to_string(),
                question_hints: vec!["What are the operating costs?".to_string()],
                topic_tags: vec!["expenses".to_string(), "costs".to_string()],
                leaf_count: 2,
                level: 1,
            },
        );

        (tree, nav, root, revenue, expenses)
    }

    #[test]
    fn test_build_ancestor_path() {
        let (tree, nav, root, revenue, _) = build_semantic_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };

        let path = build_ancestor_path(revenue, &ctx);
        assert_eq!(path, "Root > Revenue");

        let root_path = build_ancestor_path(root, &ctx);
        assert_eq!(root_path, "Root");
    }

    #[test]
    fn test_semantic_hints_keyword_match() {
        let (tree, nav, _, _, _) = build_semantic_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };

        let keywords = extract_keywords("What is the revenue?");
        let hints = build_semantic_hints(&keywords, &"what is the revenue".to_lowercase(), &ctx);

        assert!(
            hints.contains("Revenue"),
            "Should match Revenue section, got: {}",
            hints
        );
        assert!(
            hints.contains("BM25"),
            "Should include BM25 score, got: {}",
            hints
        );
    }

    #[test]
    fn test_semantic_hints_topic_match() {
        let (tree, nav, _, _, _) = build_semantic_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };

        // "costs" should match the Expenses topic_tag via BM25 scoring
        let keywords = extract_keywords("operating costs analysis");
        let hints =
            build_semantic_hints(&keywords, &"operating costs analysis".to_lowercase(), &ctx);

        assert!(
            hints.contains("Expenses"),
            "Should match Expenses section via BM25 + topic tag 'costs', got: {}",
            hints
        );
        assert!(
            hints.contains("BM25"),
            "Should include BM25 score, got: {}",
            hints
        );
    }

    #[test]
    fn test_semantic_hints_no_match() {
        let (tree, nav, _, _, _) = build_semantic_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };

        // "xyzzy" is a nonsense word that won't match any route metadata
        let keywords = extract_keywords("xyzzy foobar");
        let hints = build_semantic_hints(&keywords, &"xyzzy foobar".to_lowercase(), &ctx);

        assert!(
            hints.is_empty(),
            "Should not match anything for unrelated query, got: {}",
            hints
        );
    }

    #[test]
    fn test_build_replan_prompt() {
        let (tree, nav, root, _, _) = build_semantic_test_tree();
        let mut state = State::new(root, 8);
        state.missing_info = "Need Q2 revenue figures".to_string();
        state.add_evidence(Evidence {
            source_path: "root/Revenue".to_string(),
            node_title: "Revenue".to_string(),
            content: "Q1 revenue was $2.5M".to_string(),
            doc_name: None,
        });

        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };

        let (system, user) = build_replan_prompt("What is total revenue?", None, &state, &ctx);

        assert!(system.contains("re-planning"));
        assert!(user.contains("What is total revenue?"));
        assert!(user.contains("Q2 revenue"));
        assert!(user.contains("[Revenue]"));
        assert!(user.contains("Remaining rounds"));
    }

    #[test]
    fn test_build_plan_prompt_with_semantic_hints() {
        let (tree, nav, _, _, _) = build_semantic_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "Financial Report",
        };

        let ls_output =
            "[1] Revenue — Revenue breakdown (2 leaves)\n[2] Expenses — Cost analysis (2 leaves)\n";

        let (system, user) = build_plan_prompt(
            "What is the revenue?",
            None,
            ls_output,
            "Financial Report",
            &[],
            &ctx,
        );

        assert!(system.contains("semantic hints"));
        // "revenue" should produce BM25 matches against the Revenue route
        assert!(
            user.contains("Revenue") || user.contains("BM25") || user.contains("Semantic hints")
        );
        assert!(user.contains("What is the revenue?"));
    }

    // --- Complexity detection tests ---

    #[test]
    fn test_complexity_simple() {
        assert_eq!(
            detect_query_complexity("What is revenue?"),
            QueryComplexity::Simple
        );
        assert_eq!(
            detect_query_complexity("Define async"),
            QueryComplexity::Simple
        );
        assert_eq!(
            detect_query_complexity("什么是向量检索"),
            QueryComplexity::Simple
        );
        assert_eq!(
            detect_query_complexity("Q1 revenue"),
            QueryComplexity::Simple
        );
    }

    #[test]
    fn test_complexity_complex() {
        assert_eq!(
            detect_query_complexity(
                "Compare and contrast the different approaches to async programming"
            ),
            QueryComplexity::Complex
        );
        assert_eq!(
            detect_query_complexity("What is the relationship between ownership and borrowing?"),
            QueryComplexity::Complex
        );
        assert_eq!(
            detect_query_complexity("对比A和B的区别"),
            QueryComplexity::Complex
        );
        assert_eq!(
            detect_query_complexity("分析索引和检索的关系"),
            QueryComplexity::Complex
        );
    }

    #[test]
    fn test_complexity_multiple_questions() {
        assert_eq!(
            detect_query_complexity("What is X? How does Y work?"),
            QueryComplexity::Complex
        );
    }

    #[test]
    fn test_complexity_medium() {
        assert_eq!(
            detect_query_complexity("Show me the financial report summary"),
            QueryComplexity::Medium
        );
    }
}
