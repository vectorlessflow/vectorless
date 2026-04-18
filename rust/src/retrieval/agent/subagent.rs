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

use super::command::{parse_command, Command};
use super::config::{Config, DocContext, Evidence, Output, Step};
use super::context::FindHit;
use super::events::EventEmitter;
use super::prompts::{
    answer_synthesis, check_sufficiency, parse_sufficiency_response, subagent_dispatch,
    subagent_navigation, SynthesisParams, NavigationParams,
};
use super::state::State;
use super::tools::common;
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
        "SubAgent starting"
    );

    let mut llm_calls: u32 = 0;

    // --- Phase 0: Fast path ---
    if config.enable_fast_path {
        if let Some(output) = fast_path(query, ctx, config, emitter) {
            info!(doc = ctx.doc_name, "Fast path hit");
            emitter.emit_completed(
                output.evidence.len(),
                output.metrics.llm_calls,
                output.metrics.rounds_used,
            );
            return Ok(output);
        }
    }

    // --- Phase 1: Bird's-eye view ---
    let mut state = State::new(ctx.root(), config.max_rounds);
    let ls_result = tools::ls(ctx, &state);
    state.last_feedback = ls_result.feedback;

    // If this SubAgent was dispatched with a task, use dispatch prompt for first round
    let use_dispatch_prompt = task.is_some();

    // --- Phase 2: Navigation loop ---
    loop {
        // Budget check
        if state.remaining == 0 {
            info!(doc = ctx.doc_name, "Budget exhausted");
            break;
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
            subagent_navigation(&NavigationParams {
                query,
                task,
                breadcrumb: &state.path_str(),
                evidence_summary: &state.evidence_summary(),
                missing_info: "",
                last_feedback: &state.last_feedback,
                remaining: state.remaining,
                max_rounds: state.max_rounds,
            })
        };

        // LLM decision
        let llm_output = match llm.complete(&system, &user).await {
            Ok(output) => output,
            Err(e) => {
                warn!(doc = ctx.doc_name, error = %e, "LLM call failed in nav loop");
                state.dec_round();
                state.last_feedback = "LLM error occurred, retrying.".to_string();
                continue;
            }
        };
        llm_calls += 1;

        // Parse command
        let command = parse_command(&llm_output);
        debug!(doc = ctx.doc_name, ?command, "Parsed command");

        let round_num = config.max_rounds - state.remaining + 1;

        // Execute command
        let step = execute_command(&command, ctx, &mut state, query, llm, &mut llm_calls, emitter).await;

        // Emit round event
        let cmd_str = format!("{:?}", command);
        let success = !matches!(step, Step::ForceDone(_));
        emitter.emit_round(round_num, &cmd_str, success);

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
                state.dec_round();
            }
        }
    }

    // --- Phase 3: Answer synthesis ---
    let mut output = state.into_output(llm_calls);

    if config.enable_synthesis && !output.evidence.is_empty() {
        let evidence_text = format_evidence_for_synthesis(&output.evidence);
        let (system, user) = answer_synthesis(&SynthesisParams {
            query,
            evidence_text: &evidence_text,
            missing_info: "",
        });

        match llm.complete(&system, &user).await {
            Ok(answer) => {
                output.answer = answer.trim().to_string();
                output.metrics.llm_calls += 1;
                emitter.emit_synthesis(output.answer.len());
            }
            Err(e) => {
                warn!(doc = ctx.doc_name, error = %e, "Synthesis LLM call failed");
                output.answer = format_evidence_as_answer(&output.evidence);
            }
        }
    } else if !output.evidence.is_empty() {
        // No synthesis — just concatenate evidence
        output.answer = format_evidence_as_answer(&output.evidence);
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
fn fast_path(query: &str, ctx: &DocContext<'_>, config: &Config, emitter: &EventEmitter) -> Option<Output> {
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
        .max_by(|a, b| a.1.weight.partial_cmp(&b.1.weight).unwrap_or(std::cmp::Ordering::Equal))?;

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
            let result = match ctx.find(keyword) {
                Some(hit) => {
                    let formatted = common::format_find_result(keyword, &[hit]);
                    ToolResultLike::ok(formatted)
                }
                None => ToolResultLike::ok(format!("No results for '{}'", keyword)),
            };
            state.last_feedback = result.feedback;
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
                    let sufficient = parse_sufficiency_response(&response);
                    emitter.emit_sufficiency(sufficient, state.evidence.len());
                    if sufficient {
                        state.last_feedback =
                            "Evidence is sufficient. Use done to finish.".to_string();
                        Step::Done
                    } else {
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
    }
}

/// Minimal result-like type for internal command results (avoids importing ToolResult).
struct ToolResultLike {
    feedback: String,
}

impl ToolResultLike {
    fn ok(feedback: String) -> Self {
        Self { feedback }
    }
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
        .map(|e| format!("**{}** (at {}):\n{}", e.node_title, e.source_path, e.content))
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
