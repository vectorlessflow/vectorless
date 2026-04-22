// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Command execution — dispatch parsed Command to tool functions.

use tracing::{info, warn};

use crate::llm::LlmClient;

use super::super::command::{Command, parse_command};
use super::super::config::{DocContext, Step};
use super::super::events::EventEmitter;
use super::super::prompts::{check_sufficiency, parse_sufficiency_response};
use super::super::state::WorkerState;
use super::super::tools::worker as tools;

/// Execute a single parsed command, mutating state.
///
/// Returns a `Step` indicating whether to continue or stop.
pub async fn execute_command(
    command: &Command,
    ctx: &DocContext<'_>,
    state: &mut WorkerState,
    query: &str,
    llm: &LlmClient,
    llm_calls: &mut u32,
    emitter: &EventEmitter,
) -> Step {
    info!(
        doc = ctx.doc_name,
        command = ?command,
        "Executing tool"
    );
    match command {
        Command::Ls => {
            let result = tools::ls(ctx, state);
            info!(doc = ctx.doc_name, feedback = %truncate_log(&result.feedback), "ls result");
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::Cd { target } => {
            let result = tools::cd(target, ctx, state);
            info!(doc = ctx.doc_name, target, feedback = %truncate_log(&result.feedback), "cd result");
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::CdUp => {
            let result = tools::cd_up(ctx, state);
            info!(doc = ctx.doc_name, feedback = %truncate_log(&result.feedback), "cd_up result");
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::Cat { target } => {
            let evidence_before = state.evidence.len();
            let result = tools::cat(target, ctx, state);
            info!(doc = ctx.doc_name, target, feedback = %truncate_log(&result.feedback), "cat result");
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
                        ctx.doc_name,
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
                        output.push_str(&format!(
                            "  - {} (depth {}, weight {:.2})",
                            title, entry.depth, entry.weight
                        ));
                        if let Some(content) = ctx.cat(entry.node_id) {
                            if let Some(snippet) =
                                content_snippet(content, keyword, 150)
                            {
                                output.push_str(&format!("\n    \"{}\"", snippet));
                            }
                        }
                        output.push('\n');
                    }
                    output
                }
                None => {
                    // Fallback: search node titles (like findtree) with content snippets
                    let pattern_lower = keyword.to_lowercase();
                    let all_nodes = ctx.tree.traverse();
                    let mut results = Vec::new();
                    for node_id in &all_nodes {
                        if let Some(node) = ctx.tree.get(*node_id) {
                            if node.title.to_lowercase().contains(&pattern_lower) {
                                let depth = ctx.tree.depth(*node_id);
                                results.push((node.title.clone(), *node_id, depth));
                            }
                        }
                    }
                    if results.is_empty() {
                        format!("No results for '{}' in index or titles.", keyword)
                    } else {
                        let mut output = format!(
                            "Results for '{}' (title match, {} found):\n",
                            keyword,
                            results.len()
                        );
                        for (title, node_id, depth) in &results {
                            output.push_str(&format!(
                                "  - {} (depth {})",
                                title, depth
                            ));
                            if let Some(content) = ctx.cat(*node_id) {
                                if let Some(snippet) = content_snippet(content, keyword, 150) {
                                    output.push_str(&format!("\n    \"{}\"", snippet));
                                }
                            }
                            output.push('\n');
                        }
                        output
                    }
                }
            };
            info!(doc = ctx.doc_name, keyword, feedback = %truncate_log(&feedback), "find result");
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

            let (system, user) = check_sufficiency(query, &evidence_summary);

            info!(
                doc = ctx.doc_name,
                system = %system,
                user = %user,
                "Check prompt"
            );

            match llm.complete(&system, &user).await {
                Ok(response) => {
                    *llm_calls += 1;
                    state.check_count += 1;
                    let sufficient = parse_sufficiency_response(&response);
                    info!(
                        doc = ctx.doc_name,
                        sufficient,
                        evidence = state.evidence.len(),
                        response = %response,
                        "Sufficiency check"
                    );
                    emitter.emit_worker_sufficiency_check(
                        ctx.doc_name,
                        sufficient,
                        state.evidence.len(),
                        None,
                    );
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
            info!(doc = ctx.doc_name, pattern, feedback = %truncate_log(&result.feedback), "grep result");
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::Head { target, lines } => {
            let result = tools::head(target, *lines, ctx, state);
            info!(doc = ctx.doc_name, target, lines, feedback = %truncate_log(&result.feedback), "head result");
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::FindTree { pattern } => {
            let result = tools::find_tree(pattern, ctx);
            info!(doc = ctx.doc_name, pattern, feedback = %truncate_log(&result.feedback), "find_tree result");
            state.set_feedback(result.feedback);
            Step::Continue
        }

        Command::Wc { target } => {
            let result = tools::wc(target, ctx, state);
            info!(doc = ctx.doc_name, target, feedback = %truncate_log(&result.feedback), "wc result");
            state.set_feedback(result.feedback);
            Step::Continue
        }
    }
}

/// Extract a short content snippet around the first occurrence of `keyword`.
///
/// Returns `None` if the content is empty. If the keyword is not found,
/// returns the beginning of the content instead.
fn content_snippet(content: &str, keyword: &str, max_len: usize) -> Option<String> {
    if content.trim().is_empty() {
        return None;
    }

    let keyword_lower = keyword.to_lowercase();
    let content_lower = content.to_lowercase();

    // Find the keyword position to center the snippet around it
    let start = match content_lower.find(&keyword_lower) {
        Some(pos) => {
            // Back up a bit for context, but don't go negative
            let back = (max_len / 4).min(pos);
            pos - back
        }
        None => 0,
    };

    // Find a char boundary near `start`
    let start = content
        .char_indices()
        .find(|(i, _)| *i >= start)
        .map(|(i, _)| i)
        .unwrap_or(0);

    let end = content
        .char_indices()
        .take_while(|(i, _)| *i <= start + max_len)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(content.len());

    let snippet = content[start..end].trim();
    if snippet.is_empty() {
        return None;
    }

    let mut result = snippet.to_string();
    if end < content.len() {
        result.push_str("...");
    }
    if start > 0 {
        result = format!("...{}", result);
    }
    Some(result)
}

/// Truncate feedback for log output — keep first 300 chars to avoid noisy logs.
fn truncate_log(s: &str) -> std::borrow::Cow<'_, str> {
    const MAX: usize = 300;
    if s.len() <= MAX {
        std::borrow::Cow::Borrowed(s)
    } else {
        std::borrow::Cow::Owned(format!(
            "{}...(truncated, {} chars total)",
            &s[..MAX],
            s.len()
        ))
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
