// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Adaptive budget computation for agent navigation.

use super::types::QueryComplexity;

/// Adaptive budget for a SubAgent run, derived from query complexity and
/// document depth.
#[derive(Debug, Clone, Copy)]
pub struct Budget {
    /// Maximum navigation rounds (ls/cd/cat etc., excludes check).
    pub max_rounds: u32,
    /// Hard cap on total LLM calls per SubAgent.
    pub max_llm_calls: u32,
}

impl Budget {
    /// Compute an adaptive budget from query complexity, document depth, and
    /// the base configuration values.
    ///
    /// Logic migrated from `agent::subagent::run()` Phase 1 budget calculation.
    pub fn adaptive(
        complexity: QueryComplexity,
        doc_depth: usize,
        base_max_rounds: u32,
        base_max_llm_calls: u32,
    ) -> Self {
        let base_rounds = match complexity {
            QueryComplexity::Simple => (base_max_rounds * 6 / 10).max(4),
            QueryComplexity::Medium => base_max_rounds,
            QueryComplexity::Complex => (base_max_rounds * 15 / 10).max(10),
        };
        let base_llm = match complexity {
            QueryComplexity::Simple => (base_max_llm_calls * 6 / 10).max(6),
            QueryComplexity::Medium => base_max_llm_calls,
            QueryComplexity::Complex => (base_max_llm_calls * 14 / 10).max(12),
        };

        // Scale for deep documents on top of complexity-adjusted base.
        let adaptive_rounds = if doc_depth <= 2 {
            base_rounds
        } else {
            let extra = (doc_depth - 2) * 2;
            let capped = base_rounds + extra as u32;
            capped.min((base_rounds as f32 * 1.5).ceil() as u32)
        };

        Self {
            max_rounds: adaptive_rounds,
            max_llm_calls: base_llm,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_query() {
        let budget = Budget::adaptive(QueryComplexity::Simple, 3, 8, 15);
        assert!(budget.max_rounds < 8);
        assert!(budget.max_llm_calls < 15);
    }

    #[test]
    fn complex_query() {
        let budget = Budget::adaptive(QueryComplexity::Complex, 3, 8, 15);
        assert!(budget.max_rounds > 8);
        assert!(budget.max_llm_calls > 15);
    }

    #[test]
    fn medium_is_base() {
        let budget = Budget::adaptive(QueryComplexity::Medium, 2, 8, 15);
        assert_eq!(budget.max_rounds, 8);
        assert_eq!(budget.max_llm_calls, 15);
    }

    #[test]
    fn deep_doc_gets_more_rounds() {
        let shallow = Budget::adaptive(QueryComplexity::Medium, 2, 8, 15);
        let deep = Budget::adaptive(QueryComplexity::Medium, 6, 8, 15);
        assert!(deep.max_rounds > shallow.max_rounds);
    }
}
