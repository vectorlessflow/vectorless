// Copyright (c) 2026 vectorless devices
// SPDX-License-Identifier: Apache-2.0

//! Core types for query understanding.

use serde::{Deserialize, Serialize};

/// Query intent classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl std::fmt::Display for QueryIntent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryIntent::Factual => write!(f, "factual"),
            QueryIntent::Analytical => write!(f, "analytical"),
            QueryIntent::Navigational => write!(f, "navigational"),
            QueryIntent::Summary => write!(f, "summary"),
        }
    }
}

/// Query complexity estimation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Complexity {
    /// Single keyword, simple factoid.
    Simple,
    /// Multi-concept, requires synthesis.
    Moderate,
    /// Cross-document, comparative, or multi-faceted.
    Complex,
}

impl Default for Complexity {
    fn default() -> Self {
        Self::Simple
    }
}

impl std::fmt::Display for Complexity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Complexity::Simple => write!(f, "simple"),
            Complexity::Moderate => write!(f, "moderate"),
            Complexity::Complex => write!(f, "complex"),
        }
    }
}

/// A sub-query produced by decomposition.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
/// Produced by `QueryPipeline::understand()`. Consumed by the Orchestrator
/// and Worker agents for strategy selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryPlan {
    /// The original raw query string.
    pub original: String,
    /// Detected intent.
    pub intent: QueryIntent,
    /// Extracted keywords.
    pub keywords: Vec<String>,
    /// Key concepts identified by LLM (distinct from keywords).
    pub key_concepts: Vec<String>,
    /// Strategy hint for navigation agents.
    pub strategy_hint: String,
    /// Estimated complexity.
    pub complexity: Complexity,
    /// Rewritten queries (produced by LLM for better matching).
    pub rewritten: Vec<String>,
    /// Decomposed sub-queries (for complex/multi-faceted queries).
    pub sub_queries: Vec<SubQuery>,
}

impl QueryPlan {
    /// LLM understanding failed — produce a minimal default plan.
    pub fn default_for(query: &str, keywords: Vec<String>) -> Self {
        Self {
            original: query.to_string(),
            intent: QueryIntent::Factual,
            keywords,
            key_concepts: Vec::new(),
            strategy_hint: "focused".to_string(),
            complexity: Complexity::Simple,
            rewritten: Vec::new(),
            sub_queries: Vec::new(),
        }
    }
}
