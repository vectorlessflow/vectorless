// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM-based query complexity detection.
//!
//! Uses the Pilot's LLM client to classify query complexity.
//! Falls back to heuristic rules when LLM is unavailable or fails.

use serde::Deserialize;

use super::super::complexity::QueryComplexity;
use crate::llm::LlmClient;

/// LLM response schema for complexity classification.
#[derive(Debug, Deserialize)]
struct ComplexityResponse {
    complexity: String,
}

/// System prompt for complexity classification.
const SYSTEM_PROMPT: &str = include_str!("prompts/system_complexity.txt");
/// User prompt template.
const USER_PROMPT: &str = include_str!("prompts/user_complexity.txt");

/// Detect query complexity using LLM.
///
/// Returns `None` if the LLM call fails (caller should fall back to heuristic).
pub async fn detect_with_llm(
    client: &LlmClient,
    query: &str,
) -> Option<QueryComplexity> {
    let user = USER_PROMPT.replace("{query}", query);

    let resp: ComplexityResponse = client
        .complete_json_with_max_tokens(SYSTEM_PROMPT, &user, 80)
        .await
        .ok()?;

    let complexity = match resp.complexity.to_lowercase().as_str() {
        "simple" => QueryComplexity::Simple,
        "complex" => QueryComplexity::Complex,
        _ => QueryComplexity::Medium,
    };

    tracing::debug!(
        "LLM complexity detection: query='{}', result={:?}",
        query,
        complexity
    );

    Some(complexity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_not_empty() {
        assert!(!SYSTEM_PROMPT.is_empty());
        assert!(SYSTEM_PROMPT.contains("simple"));
        assert!(SYSTEM_PROMPT.contains("complex"));
    }

    #[test]
    fn test_user_prompt_template() {
        assert!(USER_PROMPT.contains("{query}"));
        let filled = USER_PROMPT.replace("{query}", "test query");
        assert!(filled.contains("test query"));
    }
}
