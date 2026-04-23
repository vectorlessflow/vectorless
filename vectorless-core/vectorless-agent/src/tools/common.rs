// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Common tools shared between Orchestrator and Worker (find, check, done).

use super::ToolResult;

/// Execute a `find` command — search for a keyword.
///
/// Returns formatted search results as feedback text.
pub fn format_find_result(keyword: &str, hits: &[super::super::context::FindHit]) -> String {
    if hits.is_empty() {
        return format!("No results found for '{}'", keyword);
    }

    let mut output = format!("Results for '{}':\n", keyword);
    for hit in hits {
        for entry in &hit.entries {
            output.push_str(&format!(
                "  - node (depth {}, weight {:.2})\n",
                entry.depth, entry.weight
            ));
        }
    }
    output
}

/// Execute a `check` command — evaluate evidence sufficiency.
///
/// Returns a formatted summary of current evidence for the LLM to evaluate.
pub fn format_check_prompt(evidence_summary: &str, query: &str) -> String {
    format!(
        "Please evaluate whether the collected evidence is sufficient to answer the query.\n\n\
         Query: {}\n\n\
         Evidence:\n{}\n\n\
         Is this sufficient? Answer YES or NO and briefly explain.",
        query, evidence_summary
    )
}

/// Execute a `done` command — signal loop termination.
pub fn format_done() -> ToolResult {
    ToolResult::done("Navigation complete.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_find_result_empty() {
        let result = format_find_result("nonexistent", &[]);
        assert!(result.contains("No results"));
    }

    #[test]
    fn test_format_check_prompt() {
        let prompt = format_check_prompt("- [Intro] 500 chars", "What is X?");
        assert!(prompt.contains("What is X?"));
        assert!(prompt.contains("500 chars"));
    }

    #[test]
    fn test_format_done() {
        let result = format_done();
        assert!(result.should_stop);
        assert!(result.success);
    }
}
