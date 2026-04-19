// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Query preprocessing — transforms raw query into a structured plan.
//!
//! Uses the `query` module for complexity detection, keyword extraction,
//! and budget computation.

use crate::query::{Budget, QueryPlan, detect_query_complexity};
use crate::scoring::bm25::extract_keywords;

/// Preprocess a raw query string into a structured [`QueryPlan`].
///
/// This is a zero-cost operation (no LLM calls). It performs:
/// - Complexity detection via heuristics
/// - Keyword extraction
/// - Budget computation (if document depth is provided)
pub fn preprocess(query: &str) -> QueryPlan {
    let complexity = detect_query_complexity(query);
    let keywords = extract_keywords(query);

    QueryPlan {
        original: query.to_string(),
        rewritten: Vec::new(),
        complexity,
        intent: Default::default(),
        sub_queries: Vec::new(),
        keywords,
        budget: Budget::adaptive(complexity, 0, 8, 15), // defaults, agent adjusts later
    }
}

/// Preprocess a query with known document depth for accurate budget.
pub fn preprocess_with_depth(
    query: &str,
    doc_depth: usize,
    base_rounds: u32,
    base_llm: u32,
) -> QueryPlan {
    let complexity = detect_query_complexity(query);
    let keywords = extract_keywords(query);
    let budget = Budget::adaptive(complexity, doc_depth, base_rounds, base_llm);

    QueryPlan {
        original: query.to_string(),
        rewritten: Vec::new(),
        complexity,
        intent: Default::default(),
        sub_queries: Vec::new(),
        keywords,
        budget,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::QueryComplexity;

    #[test]
    fn preprocess_simple() {
        let plan = preprocess("what is revenue?");
        assert_eq!(plan.complexity, QueryComplexity::Simple);
        assert!(!plan.keywords.is_empty());
    }

    #[test]
    fn preprocess_complex() {
        let plan = preprocess("compare market risk and operational risk in the 2024 report");
        assert_eq!(plan.complexity, QueryComplexity::Complex);
    }

    #[test]
    fn preprocess_with_depth_adjusts_budget() {
        let plan = preprocess_with_depth("analyze trends", 6, 8, 15);
        assert!(plan.budget.max_rounds > 8); // deep doc gets more rounds
    }
}
