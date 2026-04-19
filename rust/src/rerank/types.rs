// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Rerank result types.

/// Confidence level for the final answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidenceLevel {
    /// Evidence is sufficient and the answer is clear.
    High,
    /// Evidence is partial but usable.
    Medium,
    /// Evidence is insufficient; the answer may be inaccurate.
    Low,
}

impl ConfidenceLevel {
    /// Determine confidence from evidence count and answer quality.
    pub fn from_evidence(evidence_count: usize, answer_len: usize) -> Self {
        if evidence_count >= 3 && answer_len > 100 {
            Self::High
        } else if evidence_count >= 1 && answer_len > 20 {
            Self::Medium
        } else {
            Self::Low
        }
    }
}

/// Output from the rerank pipeline.
pub struct RerankOutput {
    /// Synthesized answer.
    pub answer: String,
    /// Top BM25 relevance score across all evidence.
    pub score: f32,
    /// Number of LLM calls used during synthesis/fusion.
    pub llm_calls: u32,
    /// Confidence level based on evidence quality.
    pub confidence: ConfidenceLevel,
}
