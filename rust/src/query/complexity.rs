// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Heuristic query complexity detection.
//!
//! Pure function, zero-cost (no LLM calls). Analyses the query text for
//! indicators of complexity based on keyword patterns and word count.

use super::text::estimate_word_count;
use super::types::QueryComplexity;

/// Detect query complexity using heuristics (zero-cost, no LLM call).
///
/// Migrated from `agent::subagent::detect_query_complexity`.
pub fn detect_query_complexity(query: &str) -> QueryComplexity {
    let query_lower = query.to_lowercase();
    let word_count = estimate_word_count(query);

    // Complex indicators (English + Chinese)
    let complex_indicators = [
        "compare",
        "contrast",
        "analyze",
        "evaluate",
        "synthesize",
        "explain why",
        "how does",
        "relationship between",
        "cause and effect",
        "\u{5bf9}\u{6bd4}",
        "\u{5206}\u{6790}",
        "\u{8bc4}\u{4f30}",
        "\u{7efc}\u{5408}",
        "\u{4e3a}\u{4ec0}\u{4e48}",
        "\u{539f}\u{56e0}",
        "\u{5173}\u{7cfb}",
        "\u{5f71}\u{54cd}",
        "\u{533a}\u{522b}",
        "\u{5f02}\u{540c}",
    ];
    for indicator in &complex_indicators {
        if query_lower.contains(indicator) {
            return QueryComplexity::Complex;
        }
    }

    // Simple indicators
    let simple_indicators = [
        "what is",
        "define",
        "list",
        "who",
        "when",
        "where",
        "\u{4ec0}\u{4e48}\u{662f}",
        "\u{5b9a}\u{4e49}",
        "\u{5217}\u{8868}",
        "\u{8c01}",
        "\u{4f55}\u{65f6}",
        "\u{54ea}\u{91cc}",
        "\u{5728}\u{54ea}",
    ];
    for indicator in &simple_indicators {
        if query_lower.contains(indicator) && word_count <= 15 {
            return QueryComplexity::Simple;
        }
    }

    // Multiple questions -> complex
    let question_marks = query.matches('?').count() + query.matches('\u{ff1f}').count();
    if question_marks > 1 {
        return QueryComplexity::Complex;
    }

    // Word count classification
    if word_count <= 5 {
        QueryComplexity::Simple
    } else if word_count <= 15 {
        QueryComplexity::Medium
    } else {
        QueryComplexity::Complex
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_keywords() {
        assert_eq!(
            detect_query_complexity("what is revenue?"),
            QueryComplexity::Simple
        );
    }

    #[test]
    fn complex_keywords() {
        assert_eq!(
            detect_query_complexity("compare market risk and operational risk"),
            QueryComplexity::Complex
        );
    }

    #[test]
    fn medium_by_word_count() {
        assert_eq!(
            detect_query_complexity("show me the financial report for last quarter"),
            QueryComplexity::Medium
        );
    }

    #[test]
    fn multiple_questions_are_complex() {
        // "what is" is a simple indicator and word count <= 15, so it matches
        // Simple first before reaching the multiple-questions check.
        // Use a query without simple indicators to test multi-question logic.
        assert_eq!(
            detect_query_complexity("tell me about revenue? and also profit?"),
            QueryComplexity::Complex
        );
    }

    #[test]
    fn short_query_is_simple() {
        assert_eq!(detect_query_complexity("revenue"), QueryComplexity::Simple);
    }

    #[test]
    fn chinese_complex() {
        assert_eq!(
            detect_query_complexity(
                "\u{5bf9}\u{6bd4}\u{5e02}\u{573a}\u{98ce}\u{9669}\u{548c}\u{8fd0}\u{8425}\u{98ce}\u{9669}"
            ),
            QueryComplexity::Complex
        );
    }

    #[test]
    fn chinese_simple() {
        assert_eq!(
            detect_query_complexity("\u{4ec0}\u{4e48}\u{662f}\u{8425}\u{6536}"),
            QueryComplexity::Simple
        );
    }
}
