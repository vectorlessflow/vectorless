// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Query decomposition for multi-turn retrieval.
//!
//! Complex queries are broken down into simpler sub-queries
//! that can be processed independently and then combined.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Query Decomposition                           │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │   Complex Query ──▶ [Decomposer] ──▶ [Sub-queries]              │
//! │         │                                  │                     │
//! │         │                                  ▼                     │
//! │         │                          ┌───────────────┐             │
//! │         │                          │ Sub-query 1   │             │
//! │         │                          │ Sub-query 2   │             │
//! │         │                          │ Sub-query 3   │             │
//! │         │                          └───────┬───────┘             │
//! │         │                                  │                     │
//! │         └──────────────────────────────────┼─────────────────────┘
//! │                                            ▼                     │
//! │                                   [Result Aggregator]             │
//! │                                            │                     │
//! │                                            ▼                     │
//! │                                      [Final Result]              │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::retrieval::decompose::{QueryDecomposer, DecompositionConfig};
//!
//! let decomposer = QueryDecomposer::new(config);
//! let result = decomposer.decompose("What is the architecture and how does caching work?").await?;
//!
//! for sub_query in &result.sub_queries {
//!     println!("Sub-query: {}", sub_query.text);
//! }
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::llm::{LlmClient, LlmExecutor};

/// Sub-query resulting from decomposition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubQuery {
    /// The sub-query text.
    pub text: String,
    /// Estimated complexity of this sub-query.
    pub complexity: SubQueryComplexity,
    /// Order of execution (lower = higher priority).
    pub priority: u8,
    /// Dependencies on other sub-queries (indices).
    pub depends_on: Vec<usize>,
    /// Type of sub-query.
    pub query_type: SubQueryType,
    /// Optional structural path constraint extracted from the query
    /// (e.g. "3.2", "Chapter 5"). When set, the search should start
    /// from the corresponding tree node instead of searching broadly.
    pub path_constraint: Option<String>,
}

/// Complexity level for a sub-query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubQueryComplexity {
    /// Simple keyword lookup.
    Simple,
    /// Requires understanding context.
    Medium,
    /// Requires synthesis or reasoning.
    Complex,
}

impl Default for SubQueryComplexity {
    fn default() -> Self {
        Self::Simple
    }
}

/// Type of sub-query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubQueryType {
    /// Fact lookup (who, what, when).
    Fact,
    /// Explanation (why, how).
    Explanation,
    /// Comparison (difference between).
    Comparison,
    /// Synthesis (summarize, combine).
    Synthesis,
    /// Navigation (where to find).
    Navigation,
}

impl Default for SubQueryType {
    fn default() -> Self {
        Self::Fact
    }
}

/// Result of query decomposition.
#[derive(Debug, Clone)]
pub struct DecompositionResult {
    /// Original query.
    pub original: String,
    /// Decomposed sub-queries.
    pub sub_queries: Vec<SubQuery>,
    /// Whether decomposition was needed.
    pub was_decomposed: bool,
    /// Reason for decomposition decision.
    pub reason: String,
    /// Estimated total complexity.
    pub total_complexity: f32,
}

impl DecompositionResult {
    /// Create a result without decomposition (query is simple enough).
    pub fn no_decomposition(query: &str, reason: &str) -> Self {
        Self {
            original: query.to_string(),
            sub_queries: vec![SubQuery {
                text: query.to_string(),
                complexity: SubQueryComplexity::Simple,
                priority: 0,
                depends_on: vec![],
                query_type: SubQueryType::Fact,
                path_constraint: None,
            }],
            was_decomposed: false,
            reason: reason.to_string(),
            total_complexity: 0.5,
        }
    }

    /// Check if decomposition produced multiple queries.
    pub fn is_multi_turn(&self) -> bool {
        self.sub_queries.len() > 1
    }

    /// Get execution order (topologically sorted).
    pub fn execution_order(&self) -> Vec<usize> {
        if self.sub_queries.len() <= 1 {
            return vec![0];
        }

        // Simple topological sort based on dependencies and priority
        let mut order: Vec<usize> = (0..self.sub_queries.len()).collect();
        order.sort_by(|&a, &b| {
            // First sort by dependencies (fewer dependencies first)
            let a_deps = self.sub_queries[a].depends_on.len();
            let b_deps = self.sub_queries[b].depends_on.len();
            if a_deps != b_deps {
                return a_deps.cmp(&b_deps);
            }
            // Then by priority (lower priority value first)
            self.sub_queries[a]
                .priority
                .cmp(&self.sub_queries[b].priority)
        });
        order
    }
}

/// Configuration for query decomposition.
#[derive(Debug, Clone)]
pub struct DecompositionConfig {
    /// Maximum sub-queries to generate.
    pub max_sub_queries: usize,
    /// Minimum query length to consider for decomposition.
    pub min_query_length: usize,
    /// Enable LLM-based decomposition.
    pub use_llm: bool,
    /// Threshold for decomposing (complexity score).
    pub complexity_threshold: f32,
    /// Enable dependency detection.
    pub detect_dependencies: bool,
}

impl Default for DecompositionConfig {
    fn default() -> Self {
        Self {
            max_sub_queries: 5,
            min_query_length: 20,
            use_llm: true,
            complexity_threshold: 0.7,
            detect_dependencies: true,
        }
    }
}

/// Query decomposer for multi-turn retrieval.
pub struct QueryDecomposer {
    /// Configuration.
    config: DecompositionConfig,
    /// LLM client for decomposition (optional).
    llm_client: Option<LlmClient>,
    /// LLM executor for unified execution (optional).
    llm_executor: Option<LlmExecutor>,
}

impl Default for QueryDecomposer {
    fn default() -> Self {
        Self::new(DecompositionConfig::default())
    }
}

impl QueryDecomposer {
    /// Create a new query decomposer.
    pub fn new(config: DecompositionConfig) -> Self {
        Self {
            config,
            llm_client: None,
            llm_executor: None,
        }
    }

    /// Add LLM client for enhanced decomposition.
    pub fn with_llm_client(mut self, client: LlmClient) -> Self {
        self.llm_client = Some(client);
        self
    }

    /// Add LLM executor for unified throttle/retry/fallback.
    pub fn with_llm_executor(mut self, executor: LlmExecutor) -> Self {
        self.llm_executor = Some(executor);
        self
    }

    /// Decompose a query into sub-queries.
    pub async fn decompose(&self, query: &str) -> crate::error::Result<DecompositionResult> {
        // Check if decomposition is needed
        if !self.should_decompose(query) {
            return Ok(DecompositionResult::no_decomposition(
                query,
                "Query is simple enough, no decomposition needed",
            ));
        }

        info!("Decomposing complex query: '{}'", query);

        // Try LLM-based decomposition if available
        if self.config.use_llm && (self.llm_client.is_some() || self.llm_executor.is_some()) {
            match self.llm_decompose(query).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    debug!("LLM decomposition failed, falling back to rule-based: {}", e);
                }
            }
        }

        // Fall back to rule-based decomposition
        self.rule_based_decompose(query)
    }

    /// Check if a query should be decomposed.
    fn should_decompose(&self, query: &str) -> bool {
        // Skip short queries
        if query.len() < self.config.min_query_length {
            return false;
        }

        // Calculate complexity score
        let complexity = self.calculate_complexity(query);
        complexity >= self.config.complexity_threshold
    }

    /// Calculate complexity score for a query.
    fn calculate_complexity(&self, query: &str) -> f32 {
        let mut score = 0.0;
        let query_lower = query.to_lowercase();

        // 1. Multiple questions (question marks or "and" between questions)
        let question_count = query.matches('?').count();
        score += (question_count as f32 * 0.3).min(1.0);

        // 2. Multiple clauses (indicated by conjunctions)
        let conjunctions = [" and ", " or ", " but ", " also ", " plus "];
        let conjunction_count = conjunctions
            .iter()
            .filter(|c| query_lower.contains(*c))
            .count();
        score += (conjunction_count as f32 * 0.2).min(0.6);

        // 3. Complex question words
        let complex_indicators = [
            "compare",
            "contrast",
            "difference between",
            "relationship between",
            "how does",
            "why does",
            "explain how",
            "analyze",
            "evaluate",
            "synthesize",
        ];
        for indicator in &complex_indicators {
            if query_lower.contains(indicator) {
                score += 0.2;
            }
        }

        // 4. Length factor
        let word_count = query.split_whitespace().count();
        if word_count > 15 {
            score += 0.1 * ((word_count - 15) as f32 / 10.0).min(1.0);
        }

        score.min(1.0)
    }

    /// Rule-based decomposition (no LLM).
    fn rule_based_decompose(&self, query: &str) -> crate::error::Result<DecompositionResult> {
        let mut sub_queries = Vec::new();
        let query_lower = query.to_lowercase();

        // Split on common patterns
        let patterns = [
            (" and ", " and "),
            ("? ", "? "),
            (" also ", " also "),
            (" as well as ", " as well as "),
        ];

        // Check for question splits
        if query.contains('?') {
            let parts: Vec<&str> = query.split('?').filter(|s| !s.trim().is_empty()).collect();
            for (i, part) in parts.iter().enumerate() {
                let text = format!("{}?", part.trim());
                sub_queries.push(SubQuery {
                    text,
                    complexity: self.estimate_sub_query_complexity(part),
                    priority: i as u8,
                    depends_on: vec![],
                    query_type: self.detect_query_type(part),
                    path_constraint: None,
                });
            }
        }

        // If no questions found, try conjunction split
        if sub_queries.is_empty() {
            for (pattern, _) in &patterns {
                if query_lower.contains(pattern) {
                    let parts: Vec<&str> = query.split(pattern).filter(|s| !s.trim().is_empty()).collect();
                    if parts.len() > 1 {
                        for (i, part) in parts.iter().enumerate() {
                            sub_queries.push(SubQuery {
                                text: part.trim().to_string(),
                                complexity: self.estimate_sub_query_complexity(part),
                                priority: i as u8,
                                depends_on: if i > 0 && self.config.detect_dependencies {
                                    vec![i - 1]
                                } else {
                                    vec![]
                                },
                                query_type: self.detect_query_type(part),
                                path_constraint: None,
                            });
                        }
                        break;
                    }
                }
            }
        }

        // If still no decomposition, return original
        if sub_queries.is_empty() || sub_queries.len() > self.config.max_sub_queries {
            return Ok(DecompositionResult::no_decomposition(
                query,
                "No clear decomposition patterns found",
            ));
        }

        Ok(DecompositionResult {
            original: query.to_string(),
            sub_queries,
            was_decomposed: true,
            reason: "Rule-based decomposition".to_string(),
            total_complexity: self.calculate_complexity(query),
        })
    }

    /// LLM-based decomposition.
    async fn llm_decompose(&self, query: &str) -> crate::error::Result<DecompositionResult> {
        let system = r#"You are a query decomposition expert. Break down complex queries into simpler sub-queries.

Rules:
1. Each sub-query should be answerable independently when possible
2. Preserve the original intent
3. Maximum 5 sub-queries
4. Return JSON format: {"sub_queries": [{"text": "...", "complexity": "simple|medium|complex", "priority": 0-4, "depends_on": [], "query_type": "fact|explanation|comparison|synthesis|navigation"}], "reason": "..."}

If the query is simple enough, return just one sub-query."#;

        let user = format!("Decompose this query: {}", query);

        let response = if let Some(ref executor) = self.llm_executor {
            executor.complete(system, &user).await.map_err(|e| {
                crate::error::Error::Llm(format!("LLM executor error: {}", e))
            })?
        } else if let Some(ref client) = self.llm_client {
            client.complete(system, &user).await.map_err(|e| {
                crate::error::Error::Llm(format!("LLM client error: {}", e))
            })?
        } else {
            return Err(crate::error::Error::Config(
                "No LLM client or executor configured".to_string(),
            ));
        };

        // Parse the JSON response
        #[derive(Deserialize)]
        struct DecompositionResponse {
            sub_queries: Vec<SubQuery>,
            reason: String,
        }

        let parsed: DecompositionResponse =
            serde_json::from_str(&extract_json(&response)).map_err(|e| {
                crate::error::Error::Llm(format!("Failed to parse decomposition: {}", e))
            })?;

        if parsed.sub_queries.is_empty() {
            return Ok(DecompositionResult::no_decomposition(
                query,
                "LLM returned empty decomposition",
            ));
        }

        let sub_queries: Vec<SubQuery> = parsed
            .sub_queries
            .into_iter()
            .take(self.config.max_sub_queries)
            .collect();

        Ok(DecompositionResult {
            original: query.to_string(),
            sub_queries,
            was_decomposed: true,
            reason: parsed.reason,
            total_complexity: self.calculate_complexity(query),
        })
    }

    /// Estimate complexity for a sub-query.
    fn estimate_sub_query_complexity(&self, text: &str) -> SubQueryComplexity {
        let text_lower = text.to_lowercase();

        // Check for complex indicators
        if text_lower.contains("compare")
            || text_lower.contains("contrast")
            || text_lower.contains("analyze")
            || text_lower.contains("evaluate")
            || text_lower.contains("synthesize")
        {
            return SubQueryComplexity::Complex;
        }

        // Check for medium complexity
        if text_lower.contains("how")
            || text_lower.contains("why")
            || text_lower.contains("explain")
            || text_lower.contains("describe")
        {
            return SubQueryComplexity::Medium;
        }

        SubQueryComplexity::Simple
    }

    /// Detect the type of a sub-query.
    fn detect_query_type(&self, text: &str) -> SubQueryType {
        let text_lower = text.to_lowercase();

        if text_lower.contains("compare")
            || text_lower.contains("difference")
            || text_lower.contains("versus")
            || text_lower.contains(" vs ")
        {
            return SubQueryType::Comparison;
        }

        if text_lower.contains("why")
            || text_lower.contains("how")
            || text_lower.contains("explain")
        {
            return SubQueryType::Explanation;
        }

        if text_lower.contains("summarize")
            || text_lower.contains("combine")
            || text_lower.contains("synthesize")
            || text_lower.contains("overall")
        {
            return SubQueryType::Synthesis;
        }

        if text_lower.contains("where")
            || text_lower.contains("which section")
            || text_lower.contains("find")
        {
            return SubQueryType::Navigation;
        }

        SubQueryType::Fact
    }
}

/// Extract JSON from a potentially verbose LLM response.
fn extract_json(text: &str) -> String {
    // Try to find JSON object
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                return text[start..=end].to_string();
            }
        }
    }
    text.to_string()
}

/// Result aggregator for multi-turn retrieval.
#[derive(Debug, Clone)]
pub struct SubQueryResult {
    /// The sub-query.
    pub query: SubQuery,
    /// Retrieved content.
    pub content: String,
    /// Relevance score.
    pub score: f32,
    /// Nodes that contributed to the result.
    pub source_nodes: Vec<String>,
}

/// Aggregator for combining sub-query results.
pub struct ResultAggregator {
    /// Maximum tokens in final result.
    pub max_tokens: usize,
    /// Weight by query priority.
    pub priority_weight: f32,
}

impl Default for ResultAggregator {
    fn default() -> Self {
        Self {
            max_tokens: 4000,
            priority_weight: 0.3,
        }
    }
}

impl ResultAggregator {
    /// Create a new result aggregator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Aggregate results from multiple sub-queries.
    pub fn aggregate(
        &self,
        results: &[SubQueryResult],
        decomposition: &DecompositionResult,
    ) -> String {
        if results.is_empty() {
            return String::new();
        }

        if results.len() == 1 {
            return results[0].content.clone();
        }

        // Sort by execution order and priority
        let order = decomposition.execution_order();
        let sorted_results: Vec<_> = order
            .iter()
            .filter_map(|&i| results.iter().find(|r| r.query.text == decomposition.sub_queries[i].text))
            .collect();

        // Combine results with section headers
        let mut combined = String::new();
        let mut total_tokens = 0;

        for result in sorted_results {
            let section = format!(
                "\n### {}\n\n{}\n",
                result.query.text,
                result.content
            );

            let section_tokens = section.len() / 4; // Rough estimate
            if total_tokens + section_tokens > self.max_tokens {
                // Truncate if needed
                let remaining = self.max_tokens - total_tokens;
                if remaining > 100 {
                    let end_pos = (remaining * 4).min(result.content.len());
                    combined.push_str(&format!(
                        "\n### {}\n\n{}\n",
                        result.query.text,
                        &result.content[..end_pos]
                    ));
                }
                break;
            }

            combined.push_str(&section);
            total_tokens += section_tokens;
        }

        combined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_calculation() {
        let decomposer = QueryDecomposer::default();

        // Simple query
        let simple = "What is the architecture?";
        let simple_score = decomposer.calculate_complexity(simple);
        assert!(simple_score < 0.5);

        // Complex query
        let complex = "What is the architecture and how does it compare to other systems?";
        let complex_score = decomposer.calculate_complexity(complex);
        assert!(complex_score > simple_score);
    }

    #[test]
    fn test_rule_based_decomposition() {
        let decomposer = QueryDecomposer::default();

        let result = decomposer.rule_based_decompose(
            "What is the architecture? How does caching work?",
        ).unwrap();

        assert!(result.was_decomposed);
        assert_eq!(result.sub_queries.len(), 2);
    }

    #[test]
    fn test_no_decomposition() {
        let result = DecompositionResult::no_decomposition(
            "What is this?",
            "Query is simple",
        );

        assert!(!result.was_decomposed);
        assert!(!result.is_multi_turn());
    }

    #[test]
    fn test_execution_order() {
        let mut result = DecompositionResult::no_decomposition("test", "test");
        result.sub_queries = vec![
            SubQuery {
                text: "First".to_string(),
                priority: 2,
                depends_on: vec![],
                query_type: SubQueryType::Fact,
                complexity: SubQueryComplexity::Simple,
                path_constraint: None,
            },
            SubQuery {
                text: "Second".to_string(),
                priority: 1,
                depends_on: vec![0],
                query_type: SubQueryType::Fact,
                complexity: SubQueryComplexity::Simple,
                path_constraint: None,
            },
        ];
        result.was_decomposed = true;

        let order = result.execution_order();
        assert_eq!(order, vec![0, 1]); // First should come before Second
    }

    #[test]
    fn test_query_type_detection() {
        let decomposer = QueryDecomposer::default();

        assert_eq!(
            decomposer.detect_query_type("Compare A and B"),
            SubQueryType::Comparison
        );
        assert_eq!(
            decomposer.detect_query_type("Why does this happen?"),
            SubQueryType::Explanation
        );
        assert_eq!(
            decomposer.detect_query_type("Where is the config?"),
            SubQueryType::Navigation
        );
    }

    #[test]
    fn test_result_aggregator() {
        let aggregator = ResultAggregator::new();

        let results = vec![
            SubQueryResult {
                query: SubQuery {
                    text: "First question?".to_string(),
                    priority: 0,
                    depends_on: vec![],
                    query_type: SubQueryType::Fact,
                    complexity: SubQueryComplexity::Simple,
                    path_constraint: None,
                },
                content: "Answer 1".to_string(),
                score: 0.9,
                source_nodes: vec![],
            },
            SubQueryResult {
                query: SubQuery {
                    text: "Second question?".to_string(),
                    priority: 1,
                    depends_on: vec![0],
                    query_type: SubQueryType::Fact,
                    complexity: SubQueryComplexity::Simple,
                    path_constraint: None,
                },
                content: "Answer 2".to_string(),
                score: 0.8,
                source_nodes: vec![],
            },
        ];

        let mut decomposition = DecompositionResult::no_decomposition("test", "test");
        decomposition.sub_queries = results.iter().map(|r| r.query.clone()).collect();
        decomposition.was_decomposed = true;

        let combined = aggregator.aggregate(&results, &decomposition);
        assert!(combined.contains("First question"));
        assert!(combined.contains("Answer 1"));
    }
}
