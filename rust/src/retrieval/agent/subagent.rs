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
use crate::retrieval::scoring::bm25::extract_keywords;

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
    if config.enable_fast_path {
        if let Some(output) = fast_path(query, ctx, config, emitter) {
            info!(doc = ctx.doc_name, "Fast path hit — skipping navigation");
            emitter.emit_completed(
                output.evidence.len(),
                output.metrics.llm_calls,
                output.metrics.rounds_used,
            );
            return Ok(output);
        }
        debug!(doc = ctx.doc_name, "Fast path miss — entering navigation loop");
    }

    // --- Phase 1: Bird's-eye view ---
    debug!(doc = ctx.doc_name, "Phase 1: bird's-eye view (ls root)");
    let mut state = State::new(ctx.root(), config.max_rounds);
    let ls_result = tools::ls(ctx, &state);
    state.last_feedback = ls_result.feedback;

    // --- Phase 1.5: Navigation planning ---
    // One LLM call to generate a tentative navigation plan from the bird's-eye view.
    // The plan is non-binding guidance injected into subsequent prompts.
    if state.remaining > 0 && !llm_budget_exhausted!() {
        let plan_prompt = build_plan_prompt(query, task, &state.last_feedback, ctx.doc_name);
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
                    state.plan = plan_text;
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
                llm_calls,
                max_llm,
                "LLM call budget exhausted"
            );
            break;
        }

        // Stuck detection: inject warning if no progress
        if state.rounds_since_evidence >= STUCK_THRESHOLD {
            let stuck_warning = format!(
                "\n[Warning: No new evidence collected in {} rounds. \
                 Consider using grep, findtree, or cd .. to explore a different path.]",
                state.rounds_since_evidence
            );
            if !state.last_feedback.contains("[Warning:") {
                state.last_feedback.push_str(&stuck_warning);
            }
        }

        // Mid-budget checkpoint: remind LLM to check if it hasn't yet
        let half_budget = state.max_rounds / 2;
        let rounds_used = state.max_rounds - state.remaining;
        if rounds_used == half_budget && !state.check_called && state.remaining > 1 {
            if !state.last_feedback.contains("[Hint:") {
                state.last_feedback.push_str(
                    "\n[Hint: You've used half your budget. Consider running `check` to evaluate if collected evidence is sufficient.]",
                );
            }
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

        // Emit round event
        let cmd_str = format!("{:?}", command);
        let success = !matches!(step, Step::ForceDone(_));
        emitter.emit_round(round_num, &cmd_str, success);

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
        debug!(doc = ctx.doc_name, "Synthesis disabled — concatenating raw evidence");
        output.answer = format_evidence_as_answer(&output.evidence);
    } else {
        info!(doc = ctx.doc_name, "No evidence collected — returning empty output");
    }

    emitter.emit_completed(
        output.evidence.len(),
        output.metrics.llm_calls,
        output.metrics.rounds_used,
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

/// Try the fast path: extract keywords → look up in ReasoningIndex → return if confident.
fn fast_path(
    query: &str,
    ctx: &DocContext<'_>,
    config: &Config,
    emitter: &EventEmitter,
) -> Option<Output> {
    let keywords = extract_keywords(query);
    if keywords.is_empty() {
        return None;
    }

    let hits: Vec<FindHit> = ctx.find_all(&keywords);
    if hits.is_empty() {
        return None;
    }

    // Find the best matching node
    let best_entry = hits
        .iter()
        .flat_map(|hit| hit.entries.iter().map(|e| (hit.keyword.clone(), e)))
        .max_by(|a, b| {
            a.1.weight
                .partial_cmp(&b.1.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;

    if best_entry.1.weight < config.fast_path_threshold {
        debug!(
            keyword = %best_entry.0,
            weight = best_entry.1.weight,
            threshold = config.fast_path_threshold,
            "Fast path: best hit below threshold"
        );
        return None;
    }

    // Read content from the best node
    let content = ctx.cat(best_entry.1.node_id).unwrap_or("").to_string();
    let title = ctx
        .node_title(best_entry.1.node_id)
        .unwrap_or("unknown")
        .to_string();

    if content.is_empty() {
        return None;
    }

    info!(
        keyword = %best_entry.0,
        node = %title,
        weight = best_entry.1.weight,
        "Fast path hit"
    );

    emitter.emit_fast_path(&best_entry.0, &title, best_entry.1.weight);

    Some(Output::fast_path(
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
            state.last_feedback = result.feedback;
            Step::Continue
        }

        Command::Cd { target } => {
            let result = tools::cd(target, ctx, state);
            state.last_feedback = result.feedback;
            Step::Continue
        }

        Command::CdUp => {
            let result = tools::cd_up(ctx, state);
            state.last_feedback = result.feedback;
            Step::Continue
        }

        Command::Cat { target } => {
            let evidence_before = state.evidence.len();
            let result = tools::cat(target, ctx, state);
            state.last_feedback = result.feedback;
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
                    let mut output = format!("Results for '{}':\n", keyword);
                    for entry in &hit.entries {
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
            state.last_feedback = feedback;
            Step::Continue
        }

        Command::Pwd => {
            let result = tools::pwd(state);
            state.last_feedback = result.feedback;
            Step::Continue
        }

        Command::Check => {
            let evidence_summary = state.evidence_summary();
            let (system, user) = check_sufficiency(query, &evidence_summary);

            match llm.complete(&system, &user).await {
                Ok(response) => {
                    *llm_calls += 1;
                    state.check_called = true;
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
                            // Plan failed — clear it so react decisions take over
                            state.plan.clear();
                        }
                        state.last_feedback =
                            format!("Evidence not yet sufficient: {}", response.trim());
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
            state.last_feedback = result.feedback;
            Step::Continue
        }

        Command::Head { target, lines } => {
            let result = tools::head(target, *lines, ctx, state);
            state.last_feedback = result.feedback;
            Step::Continue
        }

        Command::FindTree { pattern } => {
            let result = tools::find_tree(pattern, ctx);
            state.last_feedback = result.feedback;
            Step::Continue
        }

        Command::Wc { target } => {
            let result = tools::wc(target, ctx, state);
            state.last_feedback = result.feedback;
            Step::Continue
        }
    }
}

/// Build the navigation planning prompt (Phase 1.5).
///
/// One-shot LLM call after bird's-eye view to generate a tentative navigation plan.
fn build_plan_prompt(query: &str, task: Option<&str>, ls_output: &str, doc_name: &str) -> (String, String) {
    let task_section = match task {
        Some(t) => format!("\nYour specific task: {}", t),
        None => String::new(),
    };

    let system = "You are a document navigation planner. Given a user question and the top-level \
         document structure, output a brief navigation plan: which sections to visit and in what order. \
         The plan should be 2-5 steps. Each step should be a specific action like \
         \"cd to X, then cat Y\" or \"grep for Z in subtree\". \
         Output only the plan, nothing else.".to_string();

    let user = format!(
        "Document: {doc_name}\n\
         Top-level structure:\n{ls_output}\n\
         User question: {query}{task_section}\n\n\
         Navigation plan:"
    );

    (system, user)
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

/// Format evidence items for the synthesis prompt.
fn format_evidence_for_synthesis(evidence: &[Evidence]) -> String {
    evidence
        .iter()
        .map(|e| {
            format!(
                "[{}] (source: {})\n{}",
                e.node_title, e.source_path, e.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
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
        assert!(result.is_none());
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
        assert!(result.is_none());
    }
}
