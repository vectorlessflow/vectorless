// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Command execution — dispatch parsed Command to tool functions.

use tracing::{info, warn};

use crate::llm::LlmClient;

use super::super::command::{Command, parse_command};
use super::super::config::{DocContext, Step};
use super::super::events::EventEmitter;
use super::super::state::State;
use super::super::prompts::{check_sufficiency, parse_sufficiency_response};
use super::sufficiency::heuristic_sufficiency;
use super::super::tools::worker as tools;

/// Execute a single parsed command, mutating state.
///
/// Returns a `Step` indicating whether to continue or stop.
pub async fn execute_command(
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
                            continue;
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

/// Parse the LLM output and detect parse failures.
///
/// Returns `(command, is_parse_failure)`.
pub fn parse_and_detect_failure(llm_output: &str) -> (Command, bool) {
    let command = parse_command(llm_output);
    let trimmed = llm_output.trim();
    let is_parse_failure =
        matches!(command, Command::Ls) && !trimmed.starts_with("ls") && !trimmed.is_empty();
    (command, is_parse_failure)
}
