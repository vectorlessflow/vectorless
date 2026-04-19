// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Core types for query understanding.

/// Query complexity level for adaptive budget selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryComplexity {
    /// Simple queries that can be solved with keyword matching.
    Simple,
    /// Medium complexity queries requiring semantic understanding.
    Medium,
    /// Complex queries requiring deep LLM reasoning.
    Complex,
}

impl Default for QueryComplexity {
    fn default() -> Self {
        Self::Medium
    }
}

/// Query intent classification (future: will be populated by LLM).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryIntent {
    /// Factoid: "What is the Q3 2024 revenue?"
    Factual,
    /// Analytical: "Compare market risk vs operational risk"
    Analytical,
    /// Navigation: "Find the section on compliance policy"
    Navigational,
    /// Summary: "Summarize the main points of this document"
    Summary,
}

impl Default for QueryIntent {
    fn default() -> Self {
        Self::Factual
    }
}

/// A sub-query produced by decomposition (future: multi-doc / complex queries).
#[derive(Debug, Clone)]
pub struct SubQuery {
    /// The sub-query text.
    pub query: String,
    /// Intent of this sub-query.
    pub intent: QueryIntent,
    /// Pre-identified target documents (if any).
    pub target_docs: Option<Vec<String>>,
}

/// A structured query plan — the output of the query understanding pipeline.
///
/// This is consumed by the retrieval dispatcher and agent modules.
#[derive(Debug, Clone)]
pub struct QueryPlan {
    /// The original raw query string.
    pub original: String,
    /// Rewritten queries (currently empty; future: LLM rewrite).
    pub rewritten: Vec<String>,
    /// Detected complexity level.
    pub complexity: QueryComplexity,
    /// Detected intent.
    pub intent: QueryIntent,
    /// Decomposed sub-queries (currently empty; future: decomposition).
    pub sub_queries: Vec<SubQuery>,
    /// Extracted keywords.
    pub keywords: Vec<String>,
    /// Adaptive budget derived from complexity + document depth.
    pub budget: super::Budget,
}
