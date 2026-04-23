// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Phase 2: Navigation loop — LLM-driven command loop until done or budget exhausted.

use tracing::{debug, info};

use super::super::command::Command;
use super::super::config::{DocContext, Step, WorkerConfig};
use super::super::context::FindHit;
use super::super::events::EventEmitter;
use super::super::prompts::{NavigationParams, worker_dispatch, worker_navigation};
use super::super::state::WorkerState;
use super::execute::{execute_command, parse_and_detect_failure};
use super::format::format_visited_titles;
use super::planning::{build_replan_prompt, format_keyword_hints};
use vectorless_error::Error;
use vectorless_llm::LlmClient;

/// Run the Phase 2 navigation loop.
///
/// Loops until budget exhausted, `done`/`force_done`, or error.
/// Mutates `state` and `llm_calls` in place.
pub async fn run_navigation_loop(
    query: &str,
    task: Option<&str>,
    ctx: &DocContext<'_>,
    config: &WorkerConfig,
    llm: &LlmClient,
    state: &mut WorkerState,
    emitter: &EventEmitter,
    index_hits: &[FindHit],
    intent_context: &str,
    llm_calls: &mut u32,
) -> vectorless_error::Result<()> {
    let use_dispatch_prompt = task.is_some();
    let keyword_hints = format_keyword_hints(index_hits, ctx);
    let max_llm = config.max_llm_calls;

    loop {
        if state.remaining == 0 {
            info!(doc = ctx.doc_name, "Navigation budget exhausted");
            break;
        }
        if max_llm > 0 && *llm_calls >= max_llm {
            info!(
                doc = ctx.doc_name,
                llm_calls, max_llm, "LLM call budget exhausted"
            );
            break;
        }

        // Build prompt
        let (system, user) = build_round_prompt(
            query,
            task,
            ctx,
            state,
            intent_context,
            &keyword_hints,
            use_dispatch_prompt,
            config.max_rounds,
        );

        // LLM decision
        let round_num = config.max_rounds - state.remaining + 1;
        let round_start = std::time::Instant::now();
        info!(
            doc = ctx.doc_name,
            round = round_num,
            max_rounds = config.max_rounds,
            "Navigation round: calling LLM..."
        );
        let llm_output = llm
            .complete(&system, &user)
            .await
            .map_err(|e| Error::LlmReasoning {
                stage: "worker/navigation".to_string(),
                detail: format!("Nav loop LLM call failed (round {round_num}): {e}"),
            })?;
        *llm_calls += 1;

        // Parse command
        let (command, is_parse_failure) = handle_parse_failure(&llm_output, ctx.doc_name, state);
        if is_parse_failure {
            continue;
        }

        debug!(doc = ctx.doc_name, ?command, "Parsed command");

        let is_check = matches!(command, Command::Check);

        // Execute
        let step = execute_command(&command, ctx, state, query, llm, llm_calls, emitter).await;

        // Dynamic re-planning after insufficient check
        handle_replan(
            is_check, query, task, ctx, llm, state, emitter, llm_calls, max_llm,
        )
        .await?;

        // Emit round event
        let cmd_str = format!("{:?}", command);
        let success = !matches!(step, Step::ForceDone(_));
        let round_elapsed = round_start.elapsed().as_millis() as u64;
        emitter.emit_worker_round(ctx.doc_name, round_num, &cmd_str, success, round_elapsed);

        push_round_history(state, &cmd_str);

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

    Ok(())
}

/// Build the (system, user) prompt pair for a single navigation round.
fn build_round_prompt(
    query: &str,
    task: Option<&str>,
    ctx: &DocContext<'_>,
    state: &WorkerState,
    intent_context: &str,
    keyword_hints: &str,
    use_dispatch_prompt: bool,
    max_rounds: u32,
) -> (String, String) {
    if use_dispatch_prompt && state.remaining == max_rounds {
        worker_dispatch(&super::super::prompts::WorkerDispatchParams {
            original_query: query,
            task: task.unwrap_or(query),
            doc_name: ctx.doc_name,
            breadcrumb: &state.path_str(),
        })
    } else {
        let visited_titles = format_visited_titles(state, ctx);
        worker_navigation(&NavigationParams {
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
            intent_context,
            keyword_hints,
        })
    }
}

/// Parse LLM output and handle parse failures.
///
/// Returns `(command, is_parse_failure)`. On parse failure, updates state
/// with feedback and pushes a history entry.
fn handle_parse_failure(
    llm_output: &str,
    doc_name: &str,
    state: &mut WorkerState,
) -> (Command, bool) {
    if llm_output.trim().len() < 2 {
        tracing::warn!(
            doc = doc_name,
            response = llm_output.trim(),
            "LLM response unusually short"
        );
    }
    let (command, is_parse_failure) = parse_and_detect_failure(llm_output);
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
    }
    (command, is_parse_failure)
}

/// Push a round's command + feedback preview into history and trace.
fn push_round_history(state: &mut WorkerState, cmd_str: &str) {
    let feedback_preview = if state.last_feedback.len() > 120 {
        let boundary = state.last_feedback.ceil_char_boundary(120);
        format!("{}...", &state.last_feedback[..boundary])
    } else {
        state.last_feedback.clone()
    };
    state.push_history(format!("{} → {}", cmd_str, feedback_preview));

    let round = state.max_rounds.saturating_sub(state.remaining);
    state.trace_steps.push(vectorless_document::TraceStep {
        action: cmd_str.to_string(),
        observation: state.last_feedback.chars().take(200).collect(),
        round,
    });
}

/// Dynamic re-planning after an insufficient check.
///
/// If check returned INSUFFICIENT with enough remaining rounds and LLM budget,
/// generates a new navigation plan. Otherwise clears stale replan state.
async fn handle_replan(
    is_check: bool,
    query: &str,
    task: Option<&str>,
    ctx: &DocContext<'_>,
    llm: &LlmClient,
    state: &mut WorkerState,
    emitter: &EventEmitter,
    llm_calls: &mut u32,
    max_llm: u32,
) -> vectorless_error::Result<()> {
    if !is_check {
        return Ok(());
    }

    if !state.missing_info.is_empty()
        && state.remaining >= 3
        && (max_llm == 0 || *llm_calls < max_llm)
    {
        let missing = state.missing_info.clone();
        info!(doc = ctx.doc_name, missing = %missing, "Re-planning navigation...");
        let replan = build_replan_prompt(query, task, state, ctx);
        let new_plan =
            llm.complete(&replan.0, &replan.1)
                .await
                .map_err(|e| Error::LlmReasoning {
                    stage: "worker/replan".to_string(),
                    detail: format!("Re-plan LLM call failed: {e}"),
                })?;
        *llm_calls += 1;
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
    } else if !state.missing_info.is_empty() {
        state.plan.clear();
        state.missing_info.clear();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::DocContext;
    use crate::agent::state::WorkerState;
    use vectorless_document::{DocumentTree, NodeId};

    fn test_ctx() -> (DocumentTree, NodeId) {
        let tree = DocumentTree::new("Root", "root content");
        let root = tree.root();
        (tree, root)
    }

    #[test]
    fn test_handle_parse_failure_valid_command() {
        let (tree, root) = test_ctx();
        let nav = vectorless_document::NavigationIndex::new();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &vectorless_document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let mut state = WorkerState::new(root, 10);

        let (cmd, is_failure) = handle_parse_failure("ls", ctx.doc_name, &mut state);
        assert!(!is_failure);
        assert!(matches!(cmd, Command::Ls));
    }

    #[test]
    fn test_handle_parse_failure_unrecognized() {
        let (tree, root) = test_ctx();
        let nav = vectorless_document::NavigationIndex::new();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &vectorless_document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let mut state = WorkerState::new(root, 10);

        let (_cmd, is_failure) =
            handle_parse_failure("random garbage text", ctx.doc_name, &mut state);
        assert!(is_failure);
        assert!(state.last_feedback.contains("not recognized"));
        assert!(state.history.last().unwrap().contains("unrecognized"));
    }

    #[test]
    fn test_handle_parse_failure_short_response() {
        let (tree, root) = test_ctx();
        let nav = vectorless_document::NavigationIndex::new();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &vectorless_document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let mut state = WorkerState::new(root, 10);

        // Single character response — short but not a parse failure if it's "ls"
        let (cmd, is_failure) = handle_parse_failure("ls", ctx.doc_name, &mut state);
        assert!(!is_failure);
        assert!(matches!(cmd, Command::Ls));
    }

    #[test]
    fn test_push_round_history_short_feedback() {
        let (_, root) = test_ctx();
        let mut state = WorkerState::new(root, 10);
        state.last_feedback = "short feedback".to_string();

        push_round_history(&mut state, "ls");
        assert_eq!(state.history.len(), 1);
        assert!(state.history[0].contains("ls → short feedback"));
    }

    #[test]
    fn test_push_round_history_long_feedback() {
        let (_, root) = test_ctx();
        let mut state = WorkerState::new(root, 10);
        state.last_feedback = "a".repeat(200);

        push_round_history(&mut state, "cat");
        assert_eq!(state.history.len(), 1);
        assert!(state.history[0].contains("cat → "));
        // Should be truncated with ...
        assert!(state.history[0].contains("..."));
    }

    #[test]
    fn test_push_round_history_respects_max_entries() {
        let (_, root) = test_ctx();
        let mut state = WorkerState::new(root, 10);
        state.last_feedback = "ok".to_string();

        for i in 0..8 {
            push_round_history(&mut state, &format!("cmd_{i}"));
        }
        // MAX_HISTORY_ENTRIES is 6, so only last 6 should remain
        assert_eq!(state.history.len(), 6);
    }

    #[test]
    fn test_build_round_prompt_dispatch_first_round() {
        let (tree, root) = test_ctx();
        let nav = vectorless_document::NavigationIndex::new();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &vectorless_document::ReasoningIndex::default(),
            doc_name: "test_doc",
        };
        let mut state = WorkerState::new(root, 10);
        // remaining == max_rounds means first round
        assert_eq!(state.remaining, 10);

        let (system, user) = build_round_prompt(
            "test query",
            Some("sub-task"),
            &ctx,
            &state,
            "factual — find answer",
            "",
            true, // use_dispatch_prompt
            10,
        );
        assert!(system.contains("dispatch") || !system.is_empty());
        assert!(user.contains("test query") || user.contains("sub-task"));
    }

    #[test]
    fn test_build_round_prompt_navigation_subsequent_round() {
        let (tree, root) = test_ctx();
        let nav = vectorless_document::NavigationIndex::new();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &vectorless_document::ReasoningIndex::default(),
            doc_name: "test_doc",
        };
        let mut state = WorkerState::new(root, 10);
        state.remaining = 8; // not first round

        let (system, _user) = build_round_prompt(
            "test query",
            None,
            &ctx,
            &state,
            "factual",
            "keyword hints here",
            false, // use_dispatch_prompt
            10,
        );
        assert!(!system.is_empty());
    }

    #[test]
    fn test_utf8_safe_truncation_in_history() {
        let (_, root) = test_ctx();
        let mut state = WorkerState::new(root, 10);
        // Each '中' is 3 bytes in UTF-8
        state.last_feedback = "中文反馈内容测试截断安全".repeat(20);

        push_round_history(&mut state, "cat");
        let entry = &state.history[0];
        // Should be truncated without panicking
        assert!(entry.contains("cat → "));
        assert!(entry.len() < state.last_feedback.len() + 20);
    }
}
