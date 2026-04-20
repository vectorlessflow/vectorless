// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Formatting helpers for prompts and synthesis.

use super::super::config::Evidence;
use super::super::state::State;
use super::super::config::DocContext;

/// Maximum total characters for evidence in the synthesis prompt.
const SYNTHESIS_EVIDENCE_CAP: usize = 8000;

/// Resolve visited NodeIds to their titles for prompt injection.
pub fn format_visited_titles(state: &State, ctx: &DocContext<'_>) -> String {
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

/// Format evidence items for the synthesis prompt, with a total character cap.
pub fn format_evidence_for_synthesis(evidence: &[Evidence]) -> String {
    let mut result = String::new();
    for e in evidence {
        let item = format!(
            "[{}] (source: {})\n{}",
            e.node_title, e.source_path, e.content
        );
        if result.len() + item.len() + 2 > SYNTHESIS_EVIDENCE_CAP {
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
pub fn format_evidence_as_answer(evidence: &[Evidence]) -> String {
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
}
