// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM-driven query understanding.
//!
//! Uses an LLM call to analyze the query and produce a structured [`QueryPlan`].
//! Falls back to keyword-only analysis on LLM failure.

use serde::Deserialize;
use tracing::info;

use crate::llm::LlmClient;

use super::types::{Complexity, QueryIntent, QueryPlan, SubQuery};

/// Structured analysis returned by the LLM.
#[derive(Deserialize)]
struct QueryAnalysis {
    intent: String,
    key_concepts: Vec<String>,
    strategy_hint: String,
    complexity: String,
    rewritten: Option<String>,
    sub_queries: Vec<String>,
}

/// Use LLM to understand the query and produce a QueryPlan.
///
/// Propagates LLM errors — no silent degradation. The caller decides
/// how to handle failure (retry, abort, etc.).
pub async fn understand(
    query: &str,
    keywords: &[String],
    llm: &LlmClient,
) -> crate::error::Result<QueryPlan> {
    let (system, user) = understand_prompt(query, keywords);
    let response = llm.complete(&system, &user).await?;
    let analysis = parse_analysis(&response).ok_or_else(|| {
        crate::error::Error::Config(format!(
            "Query understanding returned unparseable response: {}",
            &response[..response.len().min(200)]
        ))
    })?;
    info!(
        intent = %analysis.intent,
        complexity = %analysis.complexity,
        concepts = analysis.key_concepts.len(),
        "Query understanding complete"
    );
    Ok(analysis.into_plan(query, keywords))
}

/// Parse the LLM's JSON response into a QueryAnalysis.
fn parse_analysis(response: &str) -> Option<QueryAnalysis> {
    let trimmed = response.trim();
    // Try to extract JSON from the response (LLM may wrap it in markdown)
    let json_str = if trimmed.starts_with("```") {
        // Strip markdown code fences
        let without_start = trimmed
            .trim_start_matches(|c| c == '`' || c == 'j' || c == 's' || c == 'o' || c == 'n');
        let without_end = without_start.trim_end_matches(|c| c == '`');
        without_end.trim()
    } else {
        trimmed
    };

    serde_json::from_str(json_str).ok()
}

impl QueryAnalysis {
    fn into_plan(self, query: &str, keywords: &[String]) -> QueryPlan {
        QueryPlan {
            original: query.to_string(),
            intent: parse_intent(&self.intent),
            keywords: keywords.to_vec(),
            key_concepts: self.key_concepts,
            strategy_hint: self.strategy_hint,
            complexity: parse_complexity(&self.complexity),
            rewritten: self.rewritten.into_iter().collect(),
            sub_queries: self
                .sub_queries
                .into_iter()
                .map(|sq| SubQuery {
                    query: sq,
                    intent: QueryIntent::Factual,
                    target_docs: None,
                })
                .collect(),
        }
    }
}

fn parse_intent(s: &str) -> QueryIntent {
    match s.to_lowercase().as_str() {
        "analytical" | "analysis" | "compare" | "comparison" => QueryIntent::Analytical,
        "navigational" | "navigation" | "find" | "locate" => QueryIntent::Navigational,
        "summary" | "summarize" | "overview" => QueryIntent::Summary,
        _ => QueryIntent::Factual,
    }
}

fn parse_complexity(s: &str) -> Complexity {
    match s.to_lowercase().as_str() {
        "complex" | "high" => Complexity::Complex,
        "moderate" | "medium" => Complexity::Moderate,
        _ => Complexity::Simple,
    }
}

/// Build the LLM prompt for query understanding.
fn understand_prompt(query: &str, keywords: &[String]) -> (String, String) {
    let system = r#"You are a query analysis engine. Analyze the user's query and respond with a JSON object containing:

- "intent": one of "factual", "analytical", "navigational", "summary"
- "key_concepts": array of the main concepts/entities in the query (distinct from keywords)
- "strategy_hint": one of "focused" (single-topic), "exploratory" (broad scan), "comparative" (cross-reference), or "summary" (aggregate)
- "complexity": one of "simple", "moderate", "complex"
- "rewritten": optional rewritten version of the query for better retrieval (null if not needed)
- "sub_queries": array of sub-query strings if the query can be decomposed (empty array if not)

Respond with ONLY the JSON object, no additional text."#;

    let user = format!(
        "Query: {}\nExtracted keywords: [{}]",
        query,
        keywords.join(", ")
    );

    (system.to_string(), user)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_intent() {
        assert_eq!(parse_intent("factual"), QueryIntent::Factual);
        assert_eq!(parse_intent("analytical"), QueryIntent::Analytical);
        assert_eq!(parse_intent("analysis"), QueryIntent::Analytical);
        assert_eq!(parse_intent("navigational"), QueryIntent::Navigational);
        assert_eq!(parse_intent("summary"), QueryIntent::Summary);
        assert_eq!(parse_intent("unknown"), QueryIntent::Factual);
    }

    #[test]
    fn test_parse_complexity() {
        assert_eq!(parse_complexity("simple"), Complexity::Simple);
        assert_eq!(parse_complexity("moderate"), Complexity::Moderate);
        assert_eq!(parse_complexity("complex"), Complexity::Complex);
        assert_eq!(parse_complexity("high"), Complexity::Complex);
        assert_eq!(parse_complexity("unknown"), Complexity::Simple);
    }

    #[test]
    fn test_parse_analysis_json() {
        let response = r#"{"intent":"factual","key_concepts":["revenue","Q3"],"strategy_hint":"focused","complexity":"simple","rewritten":null,"sub_queries":[]}"#;
        let analysis = parse_analysis(response).unwrap();
        assert_eq!(analysis.intent, "factual");
        assert_eq!(analysis.key_concepts.len(), 2);
        assert!(analysis.rewritten.is_none());
    }

    #[test]
    fn test_parse_analysis_markdown_wrapped() {
        let response = "```json\n{\"intent\":\"analytical\",\"key_concepts\":[\"risk\"],\"strategy_hint\":\"comparative\",\"complexity\":\"moderate\",\"rewritten\":\"compare risks\",\"sub_queries\":[]}\n```";
        let analysis = parse_analysis(response).unwrap();
        assert_eq!(analysis.intent, "analytical");
    }

    #[test]
    fn test_parse_analysis_invalid() {
        assert!(parse_analysis("not json").is_none());
    }

    #[test]
    fn test_default_plan() {
        let plan = QueryPlan::default_for("test query", vec!["test".to_string()]);
        assert_eq!(plan.original, "test query");
        assert_eq!(plan.intent, QueryIntent::Factual);
        assert_eq!(plan.keywords.len(), 1);
        assert!(plan.key_concepts.is_empty());
        assert!(plan.sub_queries.is_empty());
    }
}
