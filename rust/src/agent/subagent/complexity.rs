// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Query complexity detection — heuristics for adaptive budget.

use crate::query::QueryComplexity;

/// Detect query complexity using heuristics (zero-cost, no LLM call).
pub fn detect_query_complexity(query: &str) -> QueryComplexity {
    let query_lower = query.to_lowercase();
    let word_count = estimate_word_count(query);

    let complex_indicators = [
        "compare", "contrast", "analyze", "evaluate", "synthesize", "explain why", "how does",
        "relationship between", "cause and effect", "对比", "分析", "评估", "综合", "为什么", "原因",
        "关系", "影响", "区别", "异同",
    ];
    for indicator in &complex_indicators {
        if query_lower.contains(indicator) {
            return QueryComplexity::Complex;
        }
    }

    let simple_indicators = [
        "what is", "define", "list", "who", "when", "where", "什么是", "定义", "列表", "谁", "何时",
        "哪里", "在哪",
    ];
    for indicator in &simple_indicators {
        if query_lower.contains(indicator) && word_count <= 15 {
            return QueryComplexity::Simple;
        }
    }

    let question_marks = query.matches('?').count() + query.matches('？').count();
    if question_marks > 1 {
        return QueryComplexity::Complex;
    }

    if word_count <= 5 {
        QueryComplexity::Simple
    } else if word_count <= 15 {
        QueryComplexity::Medium
    } else {
        QueryComplexity::Complex
    }
}

/// Compute adaptive budget (max_rounds, max_llm_calls) from base config + query/doc signals.
pub fn compute_adaptive_budget(
    query: &str,
    doc_depth: usize,
    base_rounds: u32,
    base_llm: u32,
) -> (u32, u32) {
    let complexity = detect_query_complexity(query);

    let base_rounds = match complexity {
        QueryComplexity::Simple => (base_rounds * 6 / 10).max(4),
        QueryComplexity::Medium => base_rounds,
        QueryComplexity::Complex => (base_rounds * 15 / 10).max(10),
    };
    let base_llm = match complexity {
        QueryComplexity::Simple => (base_llm * 6 / 10).max(6),
        QueryComplexity::Medium => base_llm,
        QueryComplexity::Complex => (base_llm * 14 / 10).max(12),
    };

    let adaptive_rounds = if doc_depth <= 2 {
        base_rounds
    } else {
        let extra = (doc_depth - 2) * 2;
        let capped = base_rounds + extra as u32;
        capped.min((base_rounds as f32 * 1.5).ceil() as u32)
    };

    (adaptive_rounds, base_llm)
}

/// Estimate word count, handling both CJK and Latin text.
fn estimate_word_count(text: &str) -> usize {
    let mut count = 0usize;
    let mut in_latin_word = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if in_latin_word {
                count += 1;
                in_latin_word = false;
            }
        } else if ch.is_ascii_alphanumeric() {
            in_latin_word = true;
        } else if is_cjk_char(ch) {
            if in_latin_word {
                count += 1;
                in_latin_word = false;
            }
            count += 1;
        } else if in_latin_word {
            count += 1;
            in_latin_word = false;
        }
    }
    if in_latin_word {
        count += 1;
    }
    count
}

/// Check if a character is CJK (Chinese/Japanese/Korean).
fn is_cjk_char(ch: char) -> bool {
    let cp = ch as u32;
    (0x4E00..=0x9FFF).contains(&cp)
        || (0x3400..=0x4DBF).contains(&cp)
        || (0x20000..=0x2A6DF).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0x3000..=0x303F).contains(&cp)
        || (0x3040..=0x309F).contains(&cp)
        || (0x30A0..=0x30FF).contains(&cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_simple() {
        assert_eq!(detect_query_complexity("What is revenue?"), QueryComplexity::Simple);
        assert_eq!(detect_query_complexity("Define async"), QueryComplexity::Simple);
        assert_eq!(detect_query_complexity("什么是向量检索"), QueryComplexity::Simple);
        assert_eq!(detect_query_complexity("Q1 revenue"), QueryComplexity::Simple);
    }

    #[test]
    fn test_complexity_complex() {
        assert_eq!(
            detect_query_complexity("Compare and contrast the different approaches to async programming"),
            QueryComplexity::Complex
        );
        assert_eq!(
            detect_query_complexity("What is the relationship between ownership and borrowing?"),
            QueryComplexity::Complex
        );
        assert_eq!(detect_query_complexity("对比A和B的区别"), QueryComplexity::Complex);
        assert_eq!(detect_query_complexity("分析索引和检索的关系"), QueryComplexity::Complex);
    }

    #[test]
    fn test_complexity_multiple_questions() {
        assert_eq!(
            detect_query_complexity("What is X? How does Y work?"),
            QueryComplexity::Complex
        );
    }

    #[test]
    fn test_complexity_medium() {
        assert_eq!(
            detect_query_complexity("Show me the financial report summary"),
            QueryComplexity::Medium
        );
    }
}
