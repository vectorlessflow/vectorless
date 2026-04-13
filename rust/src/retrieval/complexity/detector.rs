// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Query complexity detector implementation.
//!
//! Uses Pilot's LLM client for accurate complexity classification when available.
//! Falls back to heuristic rules (keyword + word count) when no LLM client.

use std::collections::HashSet;

use super::QueryComplexity;

/// Query complexity detector.
///
/// Uses LLM for classification when available; falls back to heuristic rules.
pub struct ComplexityDetector {
    /// Optional LLM client for LLM-based detection.
    llm_client: Option<crate::llm::LlmClient>,
}

impl ComplexityDetector {
    /// Create a new complexity detector (heuristic only).
    pub fn new() -> Self {
        Self { llm_client: None }
    }

    /// Create with LLM client for accurate detection.
    pub fn with_llm_client(client: crate::llm::LlmClient) -> Self {
        Self {
            llm_client: Some(client),
        }
    }

    /// Detect the complexity of a query.
    ///
    /// Uses LLM when available; falls back to heuristic rules.
    pub async fn detect(&self, query: &str) -> QueryComplexity {
        if let Some(ref client) = self.llm_client {
            if let Some(complexity) = crate::retrieval::pilot::detect_with_llm(client, query).await
            {
                return complexity;
            }
            tracing::warn!("LLM complexity detection failed, falling back to heuristic");
        }
        self.detect_heuristic(query)
    }

    /// Heuristic-based fallback: keyword matching + word count.
    fn detect_heuristic(&self, query: &str) -> QueryComplexity {
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
            "对比",
            "分析",
            "评估",
            "综合",
            "为什么",
            "原因",
            "关系",
            "影响",
            "区别",
            "异同",
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
            "什么是",
            "定义",
            "列表",
            "谁",
            "何时",
            "哪里",
            "在哪",
        ];

        for indicator in &simple_indicators {
            if query_lower.contains(indicator) && word_count <= 15 {
                return QueryComplexity::Simple;
            }
        }

        // Multiple questions
        let question_marks = query.matches('?').count() + query.matches('？').count();
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

    /// Get complexity score (0.0 - 1.0).
    pub fn complexity_score(&self, complexity: QueryComplexity) -> f32 {
        match complexity {
            QueryComplexity::Simple => 0.2,
            QueryComplexity::Medium => 0.5,
            QueryComplexity::Complex => 0.8,
        }
    }

    /// Analyze query features (heuristic only, no LLM call).
    pub fn analyze(&self, query: &str) -> QueryAnalysis {
        let words: Vec<&str> = query.split_whitespace().collect();
        let unique_words: HashSet<&str> = words.iter().copied().collect();

        QueryAnalysis {
            word_count: words.len(),
            unique_word_ratio: if words.is_empty() {
                0.0
            } else {
                unique_words.len() as f32 / words.len() as f32
            },
            has_question_mark: query.contains('?') || query.contains('？'),
            question_count: query.matches('?').count() + query.matches('？').count(),
            complexity: self.detect_heuristic(query),
            complexity_score: self.complexity_score(self.detect_heuristic(query)),
        }
    }
}

impl Default for ComplexityDetector {
    fn default() -> Self {
        Self::new()
    }
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
        } else {
            if in_latin_word {
                count += 1;
                in_latin_word = false;
            }
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
        || (0x2A700..=0x2B73F).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0x2F800..=0x2FA1F).contains(&cp)
        || (0x3000..=0x303F).contains(&cp)
        || (0x3040..=0x309F).contains(&cp)
        || (0x30A0..=0x30FF).contains(&cp)
}

/// Analysis result for a query.
#[derive(Debug, Clone)]
pub struct QueryAnalysis {
    /// Total word count.
    pub word_count: usize,
    /// Ratio of unique words.
    pub unique_word_ratio: f32,
    /// Whether query contains question mark.
    pub has_question_mark: bool,
    /// Number of question marks.
    pub question_count: usize,
    /// Detected complexity level.
    pub complexity: QueryComplexity,
    /// Complexity score (0.0 - 1.0).
    pub complexity_score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_queries() {
        let detector = ComplexityDetector::new();

        assert_eq!(
            detector.detect_heuristic("What is Rust?"),
            QueryComplexity::Simple
        );
        assert_eq!(
            detector.detect_heuristic("Define async"),
            QueryComplexity::Simple
        );
        assert_eq!(
            detector.detect_heuristic("什么是向量检索"),
            QueryComplexity::Simple
        );
    }

    #[test]
    fn test_complex_queries() {
        let detector = ComplexityDetector::new();

        assert_eq!(
            detector.detect_heuristic(
                "Compare and contrast the different approaches to async programming"
            ),
            QueryComplexity::Complex
        );
        assert_eq!(
            detector.detect_heuristic("What is the relationship between ownership and borrowing?"),
            QueryComplexity::Complex
        );
        assert_eq!(
            detector.detect_heuristic("对比A和B的区别"),
            QueryComplexity::Complex
        );
        assert_eq!(
            detector.detect_heuristic("分析索引和检索的关系"),
            QueryComplexity::Complex
        );
    }

    #[test]
    fn test_medium_queries() {
        let detector = ComplexityDetector::new();

        let medium_query = "How do I implement a simple web server with error handling?";
        assert_eq!(detector.detect_heuristic(medium_query), QueryComplexity::Medium);
    }

    #[test]
    fn test_estimate_word_count() {
        assert_eq!(estimate_word_count("hello world"), 2);
        assert_eq!(estimate_word_count("什么是向量"), 4);
        assert_eq!(estimate_word_count("什么是 vector search"), 4);
    }

    #[test]
    fn test_no_llm_is_ok() {
        let detector = ComplexityDetector::new();
        assert!(detector.llm_client.is_none());
    }
}
