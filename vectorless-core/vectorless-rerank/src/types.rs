// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Rerank result types.

/// Output from the rerank pipeline.
pub struct RerankOutput {
    /// Synthesized answer.
    pub answer: String,
    /// Number of LLM calls used during synthesis/fusion.
    pub llm_calls: u32,
    /// Confidence score (0.0–1.0) — derived from LLM evaluate() result.
    pub confidence: f32,
}
