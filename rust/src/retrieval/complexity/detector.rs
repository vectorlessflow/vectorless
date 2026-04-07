// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Query complexity detector implementation.

use std::collections::HashSet;

use super::QueryComplexity;

/// Configuration for complexity detection.
#[derive(Debug, Clone)]
pub struct ComplexityConfig {
    /// Maximum words for simple query.
    pub simple_max_words: usize,
    /// Maximum words for medium query.
    pub medium_max_words: usize,
    /// Complexity indicators (words that suggest complex queries).
    pub complex_indicators: Vec<String>,
    /// Simple query indicators.
    pub simple_indicators: Vec<String>,
}

impl Default for ComplexityConfig {
    fn default() -> Self {
        Self {
            simple_max_words: 5,
            medium_max_words: 15,
            complex_indicators: vec![
                "compare".to_string(),
                "contrast".to_string(),
                "analyze".to_string(),
                "evaluate".to_string(),
                "synthesize".to_string(),
                "explain why".to_string(),
                "how does".to_string(),
                "what are the implications".to_string(),
                "relationship between".to_string(),
                "cause and effect".to_string(),
            ],
            simple_indicators: vec![
                "what is".to_string(),
                "define".to_string(),
                "list".to_string(),
                "who".to_string(),
                "when".to_string(),
                "where".to_string(),
            ],
        }
    }
}

/// Query complexity detector.
///
/// Analyzes queries to determine their complexity level,
/// which influences strategy selection.
pub struct ComplexityDetector {
    config: ComplexityConfig,
}

impl ComplexityDetector {
    /// Create a new complexity detector.
    pub fn new() -> Self {
        Self {
            config: ComplexityConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: ComplexityConfig) -> Self {
        Self { config }
    }

    /// Detect the complexity of a query.
    pub fn detect(&self, query: &str) -> QueryComplexity {
        let query_lower = query.to_lowercase();
        let word_count = query.split_whitespace().count();

        // Check for complex indicators
        for indicator in &self.config.complex_indicators {
            if query_lower.contains(indicator) {
                return QueryComplexity::Complex;
            }
        }

        // Check for simple indicators
        for indicator in &self.config.simple_indicators {
            if query_lower.contains(indicator) {
                // Simple indicator found, but check word count
                if word_count <= self.config.medium_max_words {
                    return QueryComplexity::Simple;
                }
            }
        }

        // Check for multiple questions
        let question_marks = query.matches('?').count();
        if question_marks > 1 {
            return QueryComplexity::Complex;
        }

        // Check for conjunctions suggesting multiple parts
        let conjunctions = ["and", "or", "but", "however", "although"];
        let conjunction_count = conjunctions
            .iter()
            .filter(|c| query_lower.split_whitespace().any(|w| w == **c))
            .count();

        if conjunction_count >= 2 {
            return QueryComplexity::Complex;
        }

        // Check for nested concepts
        let depth_indicators = ["in the context of", "with respect to", "regarding", "about"];
        for indicator in depth_indicators {
            if query_lower.contains(indicator) {
                return QueryComplexity::Medium;
            }
        }

        // Word count based classification
        if word_count <= self.config.simple_max_words {
            QueryComplexity::Simple
        } else if word_count <= self.config.medium_max_words {
            QueryComplexity::Medium
        } else {
            QueryComplexity::Complex
        }
    }

    /// Get complexity score (0.0 - 1.0).
    pub fn complexity_score(&self, query: &str) -> f32 {
        match self.detect(query) {
            QueryComplexity::Simple => 0.2,
            QueryComplexity::Medium => 0.5,
            QueryComplexity::Complex => 0.8,
        }
    }

    /// Analyze query features.
    pub fn analyze(&self, query: &str) -> QueryAnalysis {
        let query_lower = query.to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        let unique_words: HashSet<&str> = words.iter().copied().collect();

        QueryAnalysis {
            word_count: words.len(),
            unique_word_ratio: if words.is_empty() {
                0.0
            } else {
                unique_words.len() as f32 / words.len() as f32
            },
            has_question_mark: query.contains('?'),
            question_count: query.matches('?').count(),
            complexity: self.detect(query),
            complexity_score: self.complexity_score(query),
        }
    }
}

impl Default for ComplexityDetector {
    fn default() -> Self {
        Self::new()
    }
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

        assert_eq!(detector.detect("What is Rust?"), QueryComplexity::Simple);
        assert_eq!(detector.detect("Define async"), QueryComplexity::Simple);
        assert_eq!(detector.detect("List features"), QueryComplexity::Simple);
    }

    #[test]
    fn test_complex_queries() {
        let detector = ComplexityDetector::new();

        assert_eq!(
            detector.detect("Compare and contrast the different approaches to async programming"),
            QueryComplexity::Complex
        );
        assert_eq!(
            detector.detect("What is the relationship between ownership and borrowing?"),
            QueryComplexity::Complex
        );
    }

    #[test]
    fn test_medium_queries() {
        let detector = ComplexityDetector::new();

        // Medium length without complex indicators
        let medium_query = "How do I implement a simple web server with error handling?";
        assert_eq!(detector.detect(medium_query), QueryComplexity::Medium);
    }
}
