// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Heuristic sufficiency check — skip LLM when evidence is obviously sufficient.

/// Result of the heuristic sufficiency pre-check.
pub struct SufficiencyHint {
    /// Estimated token count (~4 chars per token).
    pub estimated_tokens: usize,
    /// Content quality score (0.0 - 1.0).
    pub quality_score: f32,
}

impl SufficiencyHint {
    /// Whether the heuristic considers evidence sufficient.
    pub fn is_sufficient(&self) -> bool {
        self.estimated_tokens >= 500 && self.quality_score > 0.5
    }
}

/// Zero-cost sufficiency check using content length and quality indicators.
pub fn heuristic_sufficiency(content: &str) -> SufficiencyHint {
    let estimated_tokens = content.len() / 4;
    let mut score = 0.0f32;

    let sentence_endings = content.matches('.').count()
        + content.matches('?').count()
        + content.matches('!').count()
        + content.matches('。').count()
        + content.matches('？').count()
        + content.matches('！').count();
    score += (sentence_endings as f32 * 0.05).min(0.3);

    let paragraphs = content.matches("\n\n").count();
    score += (paragraphs as f32 * 0.1).min(0.3);

    if content.contains(':') || content.contains('-') || content.contains('：') {
        score += 0.1;
    }

    let words: Vec<&str> = content.split_whitespace().collect();
    if words.len() > 10 {
        let unique_ratio = words.iter().collect::<std::collections::HashSet<_>>().len() as f32
            / words.len() as f32;
        score += unique_ratio * 0.3;
    }

    SufficiencyHint {
        estimated_tokens,
        quality_score: score.min(1.0),
    }
}
