// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Replan: LLM-driven re-dispatch after insufficient evidence.
//!
//! After evaluate() returns insufficient, the Orchestrator replans:
//! the LLM analyzes what's missing and decides which documents to query next.
//! This replaces the old heuristic supplement logic.

use tracing::info;

use crate::error::Error;
use crate::llm::LlmClient;
use crate::scoring::bm25::extract_keywords;

use super::super::config::Evidence;
use super::super::prompts::DispatchEntry;

/// Result of the replan phase.
pub struct ReplanResult {
    /// New dispatch targets for the next round.
    pub dispatches: Vec<DispatchEntry>,
    /// The LLM's reasoning about what was missing.
    pub reasoning: String,
}

/// Replan dispatch targets based on missing information.
///
/// The LLM reviews:
/// - The original query
/// - What evidence has been collected so far
/// - What information is still missing
/// - Available documents that haven't been dispatched yet
///
/// Returns new dispatch targets. LLM errors propagate.
pub async fn replan(
    query: &str,
    missing_info: &str,
    collected_evidence: &[Evidence],
    dispatched_indices: &[usize],
    total_docs: usize,
    doc_cards_text: &str,
    llm: &LlmClient,
) -> crate::error::Result<ReplanResult> {
    let evidence_summary = format_evidence_context(collected_evidence);
    let keywords = extract_keywords(query);
    let find_text = if keywords.is_empty() {
        String::new()
    } else {
        format!("\nExtracted keywords: {}", keywords.join(", "))
    };

    let (system, user) = replan_prompt(
        query,
        missing_info,
        &evidence_summary,
        dispatched_indices,
        doc_cards_text,
        &find_text,
    );

    info!(evidence = collected_evidence.len(), "Replanning dispatch targets...");
    let response = llm
        .complete(&system, &user)
        .await
        .map_err(|e| Error::LlmReasoning {
            stage: "orchestrator/replan".to_string(),
            detail: format!("Replan LLM call failed: {e}"),
        })?;

    info!(
        response_len = response.len(),
        "Replan LLM response received"
    );

    let dispatches = parse_replan_response(&response, total_docs, dispatched_indices);
    let reasoning = response.lines().take(3).collect::<Vec<_>>().join(" ");

    info!(
        new_dispatches = dispatches.len(),
        "Replan produced new dispatch targets"
    );

    Ok(ReplanResult {
        dispatches,
        reasoning,
    })
}

/// Format collected evidence for the replan prompt.
/// Includes content so the LLM can reason about what's actually been found.
fn format_evidence_context(evidence: &[Evidence]) -> String {
    if evidence.is_empty() {
        return "(no evidence collected)".to_string();
    }
    evidence
        .iter()
        .map(|e| {
            let doc = e.doc_name.as_deref().unwrap_or("unknown");
            format!("[{}] (from {})\n{}", e.node_title, doc, e.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Build the replan prompt.
fn replan_prompt(
    query: &str,
    missing_info: &str,
    evidence_summary: &str,
    dispatched: &[usize],
    doc_cards: &str,
    keywords_text: &str,
) -> (String, String) {
    let dispatched_set: Vec<String> = dispatched
        .iter()
        .map(|&i| format!("doc {}", i + 1))
        .collect();
    let dispatched_text = if dispatched_set.is_empty() {
        "None".to_string()
    } else {
        dispatched_set.join(", ")
    };

    let system = "You are a multi-document retrieval coordinator. The first round of evidence \
         collection was insufficient to fully answer the query. Review what was collected, \
         what's missing, and decide which additional documents to query.

Output format — for each additional document to query, output a block:
- doc: <number>
  reason: <why this document may have the missing information>
  task: <what specific information to find>

Only include documents not yet dispatched. If no additional documents are likely to help, \
respond with: NO_ADDITIONAL_DOCS"
        .to_string();

    let user = format!(
        "Original question: {query}

Missing information: {missing_info}

Collected evidence so far:
{evidence_summary}

Already dispatched documents: {dispatched_text}

Available documents (all):
{doc_cards}{keywords_text}

Additional documents to query:"
    );

    (system, user)
}

/// Parse the replan response into dispatch entries.
fn parse_replan_response(
    response: &str,
    total_docs: usize,
    dispatched: &[usize],
) -> Vec<DispatchEntry> {
    let trimmed = response.trim();

    if trimmed.starts_with("NO_ADDITIONAL_DOCS") {
        return Vec::new();
    }

    let mut entries = Vec::new();
    let mut current_doc_idx: Option<usize> = None;
    let mut current_reason = String::new();
    let mut current_task = String::new();

    for line in trimmed.lines() {
        let line = line.trim();

        if let Some(rest) = line.strip_prefix("- doc:") {
            // Flush previous
            if let Some(idx) = current_doc_idx.take() {
                entries.push(DispatchEntry {
                    doc_idx: idx,
                    reason: std::mem::take(&mut current_reason),
                    task: std::mem::take(&mut current_task),
                });
            }

            let doc_num: usize = rest.trim().trim_end_matches(',').parse().unwrap_or(0);
            if doc_num > 0 && doc_num <= total_docs {
                let idx = doc_num - 1;
                // Only include if not already dispatched
                if !dispatched.contains(&idx) {
                    current_doc_idx = Some(idx);
                }
            }
        } else if let Some(rest) = line.strip_prefix("reason:") {
            current_reason = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("task:") {
            current_task = rest.trim().to_string();
        }
    }

    // Flush last
    if let Some(idx) = current_doc_idx {
        entries.push(DispatchEntry {
            doc_idx: idx,
            reason: current_reason,
            task: current_task,
        });
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_replan_response_basic() {
        let response = "\
- doc: 3
  reason: May contain the missing financial data
  task: Find Q4 revenue figures";
        let entries = parse_replan_response(response, 5, &[0, 1]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].doc_idx, 2);
        assert_eq!(entries[0].task, "Find Q4 revenue figures");
    }

    #[test]
    fn test_parse_replan_response_already_dispatched() {
        let response = "\
- doc: 1
  reason: Already queried
  task: test";
        let entries = parse_replan_response(response, 3, &[0]);
        assert!(entries.is_empty()); // doc 1 (idx 0) already dispatched
    }

    #[test]
    fn test_parse_replan_response_no_additional() {
        let response = "NO_ADDITIONAL_DOCS";
        let entries = parse_replan_response(response, 3, &[0, 1]);
        assert!(entries.is_empty());
    }
}
