// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Analyze Stage - Query analysis and information extraction.
//!
//! This stage analyzes the query to determine:
//! - Query complexity (Simple/Medium/Complex)
//! - Keywords for matching
//! - Target sections based on ToC matching
//! - Query decomposition for complex queries

use async_trait::async_trait;
use tracing::info;

use crate::document::{DocumentTree, NodeId, TocView};
use crate::retrieval::complexity::ComplexityDetector;
use crate::retrieval::decompose::{DecompositionConfig, QueryDecomposer};
use crate::retrieval::pipeline::{FailurePolicy, PipelineContext, RetrievalStage, StageOutcome};
use crate::retrieval::types::{NavigationDecision, StageName};

/// Analyze Stage - analyzes queries for retrieval planning.
///
/// This stage:
/// 1. Detects query complexity (Simple/Medium/Complex)
/// 2. Extracts keywords for matching
/// 3. Matches target sections from ToC
/// 4. Decomposes complex queries into sub-queries (if enabled)
///
/// # Example
///
/// Convert Chinese number string to integer (e.g. "三" → 3, "二十一" → 21).
fn chinese_num_to_int(s: &str) -> Option<usize> {
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return None;
    }
    // If purely digits, parse directly
    if chars.iter().all(|c| c.is_ascii_digit()) {
        return s.parse().ok();
    }
    let map = |c: char| -> usize {
        match c {
            '一' => 1,
            '二' => 2,
            '三' => 3,
            '四' => 4,
            '五' => 5,
            '六' => 6,
            '七' => 7,
            '八' => 8,
            '九' => 9,
            '十' => 10,
            '百' => 100,
            _ => 0,
        }
    };
    // Simple two-pass: handle 十/百 as positional
    let mut total: usize = 0;
    let mut current: usize = 0;
    for &c in &chars {
        let v = map(c);
        if v == 0 {
            continue;
        }
        if v >= 10 {
            // Positional multiplier
            let base = if current == 0 { 1 } else { current };
            total += base * v;
            current = 0;
        } else {
            current = v;
        }
    }
    total += current;
    if total > 0 { Some(total) } else { None }
}

/// Analyze Stage - analyzes queries for retrieval planning.
///
/// This stage:
/// 1. Detects query complexity (Simple/Medium/Complex)
/// 2. Extracts keywords for matching
/// 3. Matches target sections from ToC
/// 4. Extracts structural path hints (Section 3.2, 第3章, etc.)
/// 5. Decomposes complex queries into sub-queries (if enabled)
///
/// # Example
///
/// ```rust,ignore
/// let stage = AnalyzeStage::new()
///     .with_toc_matching(true)
///     .with_decomposition(true);
/// ```
pub struct AnalyzeStage {
    complexity_detector: ComplexityDetector,
    toc_view: TocView,
    enable_toc_matching: bool,
    /// Query decomposer for complex queries.
    query_decomposer: Option<QueryDecomposer>,
    /// Enable query decomposition.
    enable_decomposition: bool,
    /// Complexity threshold for triggering decomposition.
    decomposition_threshold: f32,
}

impl Default for AnalyzeStage {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyzeStage {
    /// Create a new analyze stage.
    pub fn new() -> Self {
        Self {
            complexity_detector: ComplexityDetector::new(),
            toc_view: TocView::new(),
            enable_toc_matching: true,
            query_decomposer: None,
            enable_decomposition: false,
            decomposition_threshold: 0.6,
        }
    }

    /// Enable or disable ToC section matching.
    pub fn with_toc_matching(mut self, enable: bool) -> Self {
        self.enable_toc_matching = enable;
        self
    }

    /// Enable query decomposition with default configuration.
    pub fn with_decomposition(mut self, enable: bool) -> Self {
        self.enable_decomposition = enable;
        if enable && self.query_decomposer.is_none() {
            self.query_decomposer = Some(QueryDecomposer::new(DecompositionConfig::default()));
        }
        self
    }

    /// Enable query decomposition with custom configuration.
    pub fn with_decomposition_config(mut self, config: DecompositionConfig) -> Self {
        self.enable_decomposition = true;
        self.query_decomposer = Some(QueryDecomposer::new(config));
        self
    }

    /// Enable query decomposition and LLM-based complexity detection.
    pub fn with_llm_client(mut self, client: crate::llm::LlmClient) -> Self {
        // Use LLM client for complexity detection
        self.complexity_detector = ComplexityDetector::with_llm_client(client.clone());
        // Also enable query decomposition
        if self.query_decomposer.is_none() {
            self.query_decomposer =
                Some(QueryDecomposer::new(DecompositionConfig::default()).with_llm_client(client));
        } else if let Some(ref mut decomposer) = self.query_decomposer {
            *decomposer =
                QueryDecomposer::new(DecompositionConfig::default()).with_llm_client(client);
        }
        self.enable_decomposition = true;
        self
    }

    /// Set complexity threshold for triggering decomposition.
    pub fn with_decomposition_threshold(mut self, threshold: f32) -> Self {
        self.decomposition_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Extract keywords from a query.
    fn extract_keywords(&self, query: &str) -> Vec<String> {
        // Simple keyword extraction:
        // 1. Lowercase
        // 2. Split on whitespace
        // 3. Remove common stop words
        // 4. Remove short words (< 2 chars)
        // 5. Remove punctuation

        let stop_words = [
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has",
            "had", "do", "does", "did", "will", "would", "could", "should", "may", "might", "must",
            "shall", "can", "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with",
            "at", "by", "from", "as", "into", "through", "during", "before", "after", "above",
            "below", "between", "under", "again", "further", "then", "once", "here", "there",
            "when", "where", "why", "how", "all", "each", "few", "more", "most", "other", "some",
            "such", "no", "nor", "not", "only", "own", "same", "so", "than", "too", "very", "just",
            "and", "but", "if", "or", "because", "until", "while", "although", "though", "what",
            "which", "who", "whom", "this", "that", "these", "those", "am", "it", "its", "itself",
            "he", "him", "his", "she", "her", "hers", "they", "them", "their", "we", "us", "our",
            "you", "your", "i", "me", "my",
        ];

        query
            .to_lowercase()
            .split_whitespace()
            .filter(|word| {
                let word = word.trim_matches(|c: char| !c.is_alphanumeric());
                word.len() >= 2 && !stop_words.contains(&word)
            })
            .map(|word| {
                word.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_string()
            })
            .filter(|word| !word.is_empty())
            .collect()
    }

    /// Extract structural path hints from the query.
    ///
    /// Recognizes patterns like:
    /// - "第3章", "第2节", "第一章" (Chinese chapter/section)
    /// - "Section 3.2", "section 4.1.2" (English section numbers)
    /// - "Chapter 5", "chapter 10" (English chapter)
    /// - "3.2.1", "2.1" (bare section numbers)
    /// - "表3", "Table 5", "图2", "Figure 4" (table/figure references)
    ///
    /// Maps them to tree NodeIds via `find_by_structure()`.
    fn extract_structure_hints(&self, query: &str, tree: &DocumentTree) -> Vec<(String, NodeId)> {
        let mut hints = Vec::new();

        // Chinese patterns: 第X章, 第X节, 第X部分
        for cap in regex::Regex::new(r"第([一二三四五六七八九十百\d]+)[章节部分]")
            .unwrap()
            .captures_iter(query)
        {
            let num = chinese_num_to_int(&cap[1]).unwrap_or(0);
            if num > 0 {
                if let Some(node_id) = tree.find_by_structure(&num.to_string()) {
                    hints.push((cap[0].to_string(), node_id));
                }
            }
        }

        // "Section X.Y.Z" or "section X.Y"
        for cap in regex::Regex::new(r"(?i)section\s+(\d+(?:\.\d+)*)")
            .unwrap()
            .captures_iter(query)
        {
            if let Some(node_id) = tree.find_by_structure(&cap[1]) {
                hints.push((cap[0].to_string(), node_id));
            }
        }

        // "Chapter X"
        for cap in regex::Regex::new(r"(?i)chapter\s+(\d+)")
            .unwrap()
            .captures_iter(query)
        {
            if let Some(node_id) = tree.find_by_structure(&cap[1]) {
                hints.push((cap[0].to_string(), node_id));
            }
        }

        // Bare section numbers: "3.2.1", "2.1"
        // Use word boundary instead of lookbehind (Rust regex doesn't support lookaround)
        for cap in regex::Regex::new(r"\b(\d+\.\d+(?:\.\d+)*)")
            .unwrap()
            .captures_iter(query)
        {
            if let Some(node_id) = tree.find_by_structure(&cap[1]) {
                hints.push((cap[0].to_string(), node_id));
            }
        }

        // Table/Figure references
        for cap in regex::Regex::new(r"(?:表|(?i)table)\s*(\d+)")
            .unwrap()
            .captures_iter(query)
        {
            if let Some(node_id) = tree.find_by_structure(&format!("table {}", &cap[1])) {
                hints.push((cap[0].to_string(), node_id));
            }
        }
        for cap in regex::Regex::new(r"(?:图|(?i)figure)\s*(\d+)")
            .unwrap()
            .captures_iter(query)
        {
            if let Some(node_id) = tree.find_by_structure(&format!("figure {}", &cap[1])) {
                hints.push((cap[0].to_string(), node_id));
            }
        }

        // Deduplicate by NodeId
        let mut seen = std::collections::HashSet::new();
        hints.retain(|(_, nid)| seen.insert(*nid));

        hints
    }

    /// Match target sections from ToC.
    fn match_toc_sections(&self, query: &str, tree: &DocumentTree) -> Vec<String> {
        if !self.enable_toc_matching {
            return Vec::new();
        }

        let toc = self.toc_view.generate_from(tree, tree.root());
        let query_lower = query.to_lowercase();

        // Find sections that match query keywords
        let mut matches: Vec<(String, f32)> = Vec::new();

        fn collect_sections(
            nodes: &[crate::document::TocNode],
            query_lower: &str,
            matches: &mut Vec<(String, f32)>,
        ) {
            for node in nodes {
                let title_lower = node.title.to_lowercase();

                // Calculate match score
                let mut score = 0.0f32;

                // Exact title match
                if title_lower.contains(query_lower) {
                    score = 1.0;
                } else {
                    // Partial word matches
                    for word in query_lower.split_whitespace() {
                        if title_lower.contains(word) {
                            score += 0.3;
                        }
                    }
                }

                if score > 0.0 {
                    matches.push((node.title.clone(), score));
                }

                // Recurse into children
                collect_sections(&node.children, query_lower, matches);
            }
        }

        collect_sections(&toc.children, &query_lower, &mut matches);

        // Sort by score and return top sections
        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        matches
            .into_iter()
            .take(5)
            .map(|(title, _)| title)
            .collect()
    }
}

#[async_trait]
impl RetrievalStage for AnalyzeStage {
    fn name(&self) -> &'static str {
        "analyze"
    }

    fn priority(&self) -> i32 {
        10 // First stage
    }

    fn failure_policy(&self) -> FailurePolicy {
        FailurePolicy::fail() // Must succeed
    }

    async fn execute(&self, ctx: &mut PipelineContext) -> crate::error::Result<StageOutcome> {
        info!("Analyzing query: '{}'", ctx.query);

        // 1. Detect complexity (LLM-based when available, heuristic fallback)
        ctx.complexity = Some(self.complexity_detector.detect(&ctx.query).await);
        info!("Query complexity: {:?}", ctx.complexity);

        // 2. Extract keywords
        ctx.keywords = self.extract_keywords(&ctx.query);
        info!("Extracted keywords: {:?}", ctx.keywords);

        // 3. Match target sections
        ctx.target_sections = self.match_toc_sections(&ctx.query, &ctx.tree);
        if !ctx.target_sections.is_empty() {
            info!("Target sections: {:?}", ctx.target_sections);
        }

        // 3.5 Extract structural path hints
        ctx.resolved_path_hints = self.extract_structure_hints(&ctx.query, &ctx.tree);
        if !ctx.resolved_path_hints.is_empty() {
            info!(
                "Resolved {} structure hints: {:?}",
                ctx.resolved_path_hints.len(),
                ctx.resolved_path_hints
                    .iter()
                    .map(|(s, _)| s)
                    .collect::<Vec<_>>()
            );
        }

        // 4. Decompose query if enabled and complex enough
        if self.enable_decomposition {
            if let Some(ref decomposer) = self.query_decomposer {
                let complexity_score = ctx
                    .complexity
                    .as_ref()
                    .map(|c| match c {
                        crate::retrieval::types::QueryComplexity::Simple => 0.3,
                        crate::retrieval::types::QueryComplexity::Medium => 0.6,
                        crate::retrieval::types::QueryComplexity::Complex => 0.9,
                    })
                    .unwrap_or(0.5);

                if complexity_score >= self.decomposition_threshold {
                    info!("Decomposing query (complexity: {:.2})", complexity_score);
                    match decomposer.decompose(&ctx.query).await {
                        Ok(result) => {
                            if result.was_decomposed {
                                info!(
                                    "Query decomposed into {} sub-queries",
                                    result.sub_queries.len()
                                );
                                for (i, sq) in result.sub_queries.iter().enumerate() {
                                    info!(
                                        "  Sub-query {}: {} (priority: {})",
                                        i, sq.text, sq.priority
                                    );
                                }
                            }
                            ctx.decomposition = Some(result);
                        }
                        Err(e) => {
                            info!(
                                "Query decomposition failed: {}, continuing with original query",
                                e
                            );
                        }
                    }
                }
            }
        }

        // 5. Update metrics
        ctx.metrics.llm_calls += 0; // No LLM calls in this stage

        // 6. Record reasoning
        let complexity_str = format!("{:?}", ctx.complexity.unwrap_or_default());
        let mut reasoning_parts = vec![
            format!("Query complexity: {}", complexity_str),
            format!("Keywords: {:?}", ctx.keywords),
        ];
        if !ctx.target_sections.is_empty() {
            reasoning_parts.push(format!("Target sections: {:?}", ctx.target_sections));
        }
        if !ctx.resolved_path_hints.is_empty() {
            reasoning_parts.push(format!(
                "Structure hints: {:?}",
                ctx.resolved_path_hints
                    .iter()
                    .map(|(s, _)| s)
                    .collect::<Vec<_>>()
            ));
        }
        if let Some(ref decomp) = ctx.decomposition {
            if decomp.was_decomposed {
                reasoning_parts.push(format!(
                    "Decomposed into {} sub-queries",
                    decomp.sub_queries.len()
                ));
            }
        }
        ctx.record_reasoning(
            StageName::Analyze,
            reasoning_parts.join("; "),
            NavigationDecision::ExploreMore,
        );

        Ok(StageOutcome::cont())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_keywords() {
        let stage = AnalyzeStage::new();

        let keywords = stage.extract_keywords("What is the architecture of the system?");
        assert!(!keywords.contains(&"the".to_string()));
        assert!(keywords.contains(&"architecture".to_string()));
        assert!(keywords.contains(&"system".to_string()));
    }

    #[test]
    fn test_extract_keywords_empty() {
        let stage = AnalyzeStage::new();
        let keywords = stage.extract_keywords("");
        assert!(keywords.is_empty());
    }

    #[test]
    fn test_extract_keywords_stopwords() {
        let stage = AnalyzeStage::new();
        let keywords = stage.extract_keywords("the a an is are was were");
        assert!(keywords.is_empty());
    }
}
