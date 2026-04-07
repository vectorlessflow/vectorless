// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration types for Pilot.
//!
//! This module defines all configuration structures that control
//! Pilot's behavior, including budget limits, intervention thresholds,
//! and operation modes.

use serde::{Deserialize, Serialize};

/// Main Pilot configuration.
///
/// Controls all aspects of Pilot behavior including budget,
/// intervention strategy, and feature flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PilotConfig {
    /// Operation mode controlling how aggressively Pilot intervenes.
    pub mode: PilotMode,
    /// Token and call budget constraints.
    pub budget: BudgetConfig,
    /// Intervention threshold settings.
    pub intervention: InterventionConfig,
    /// Whether to provide guidance at search start.
    pub guide_at_start: bool,
    /// Whether to provide guidance during backtracking.
    pub guide_at_backtrack: bool,
    /// Optional path to custom prompt templates.
    pub prompt_template_path: Option<String>,
}

impl Default for PilotConfig {
    fn default() -> Self {
        Self {
            mode: PilotMode::Balanced,
            budget: BudgetConfig::default(),
            intervention: InterventionConfig::default(),
            guide_at_start: true,
            guide_at_backtrack: true,
            prompt_template_path: None,
        }
    }
}

impl PilotConfig {
    /// Create a new config with specified mode.
    pub fn with_mode(mode: PilotMode) -> Self {
        Self {
            mode,
            ..Default::default()
        }
    }

    /// Create a high-quality config (more LLM calls).
    pub fn high_quality() -> Self {
        Self {
            mode: PilotMode::Aggressive,
            budget: BudgetConfig {
                max_tokens_per_query: 5000,
                max_tokens_per_call: 1000,
                max_calls_per_query: 10,
                max_calls_per_level: 3,
                hard_limit: false,
            },
            intervention: InterventionConfig {
                fork_threshold: 2,
                score_gap_threshold: 0.2,
                low_score_threshold: 0.4,
                max_interventions_per_level: 3,
            },
            guide_at_start: true,
            guide_at_backtrack: true,
            prompt_template_path: None,
        }
    }

    /// Create a low-cost config (fewer LLM calls).
    pub fn low_cost() -> Self {
        Self {
            mode: PilotMode::Conservative,
            budget: BudgetConfig {
                max_tokens_per_query: 500,
                max_tokens_per_call: 200,
                max_calls_per_query: 2,
                max_calls_per_level: 1,
                hard_limit: true,
            },
            intervention: InterventionConfig {
                fork_threshold: 5,
                score_gap_threshold: 0.1,
                low_score_threshold: 0.2,
                max_interventions_per_level: 1,
            },
            guide_at_start: false,
            guide_at_backtrack: true,
            prompt_template_path: None,
        }
    }

    /// Create a pure algorithm config (no LLM calls).
    pub fn algorithm_only() -> Self {
        Self {
            mode: PilotMode::AlgorithmOnly,
            ..Default::default()
        }
    }
}

/// Pilot operation mode.
///
/// Controls the trade-off between LLM usage and algorithm-only search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PilotMode {
    /// Aggressive mode: frequent LLM calls for maximum accuracy.
    Aggressive,
    /// Balanced mode: LLM calls at key decision points (default).
    #[default]
    Balanced,
    /// Conservative mode: minimal LLM calls, rely more on algorithm.
    Conservative,
    /// Pure algorithm mode: no LLM calls at all.
    AlgorithmOnly,
}

impl PilotMode {
    /// Check if this mode uses LLM at all.
    pub fn uses_llm(&self) -> bool {
        !matches!(self, PilotMode::AlgorithmOnly)
    }

    /// Get the fork threshold multiplier for this mode.
    pub fn fork_threshold_multiplier(&self) -> f32 {
        match self {
            PilotMode::Aggressive => 0.5, // Lower threshold = more interventions
            PilotMode::Balanced => 1.0,
            PilotMode::Conservative => 2.0, // Higher threshold = fewer interventions
            PilotMode::AlgorithmOnly => f32::MAX,
        }
    }
}

/// Token and call budget configuration.
///
/// Controls resource consumption during retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    /// Maximum total tokens per query (input + output).
    pub max_tokens_per_query: usize,
    /// Maximum tokens per single LLM call.
    pub max_tokens_per_call: usize,
    /// Maximum number of LLM calls per query.
    pub max_calls_per_query: usize,
    /// Maximum number of LLM calls per tree level.
    pub max_calls_per_level: usize,
    /// Whether to enforce hard limits (true) or soft limits with warnings (false).
    pub hard_limit: bool,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_tokens_per_query: 2000,
            max_tokens_per_call: 500,
            max_calls_per_query: 5,
            max_calls_per_level: 2,
            hard_limit: true,
        }
    }
}

impl BudgetConfig {
    /// Check if a given token count is within budget.
    pub fn is_within_budget(&self, used: usize) -> bool {
        used < self.max_tokens_per_query
    }

    /// Get remaining tokens given current usage.
    pub fn remaining_tokens(&self, used: usize) -> usize {
        self.max_tokens_per_query.saturating_sub(used)
    }
}

/// Intervention threshold configuration.
///
/// Controls when Pilot decides to intervene in the search process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterventionConfig {
    /// Minimum number of candidates to trigger fork intervention.
    pub fork_threshold: usize,
    /// Score gap threshold (intervene when top scores are within this range).
    pub score_gap_threshold: f32,
    /// Low score threshold (intervene when best score is below this).
    pub low_score_threshold: f32,
    /// Maximum interventions allowed per tree level.
    pub max_interventions_per_level: usize,
}

impl Default for InterventionConfig {
    fn default() -> Self {
        Self {
            fork_threshold: 3,
            score_gap_threshold: 0.15,
            low_score_threshold: 0.3,
            max_interventions_per_level: 2,
        }
    }
}

impl InterventionConfig {
    /// Check if the candidate count triggers intervention.
    pub fn should_intervene_at_fork(&self, candidate_count: usize) -> bool {
        candidate_count > self.fork_threshold
    }

    /// Check if scores are too close (algorithm uncertain).
    pub fn scores_are_close(&self, scores: &[f32]) -> bool {
        if scores.len() < 2 {
            return false;
        }
        let max_score = scores.iter().cloned().fold(0.0, f32::max);
        let min_score = scores.iter().cloned().fold(1.0, f32::min);
        (max_score - min_score) < self.score_gap_threshold
    }

    /// Check if the best score is too low.
    pub fn is_low_confidence(&self, best_score: f32) -> bool {
        best_score < self.low_score_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pilot_mode_uses_llm() {
        assert!(PilotMode::Aggressive.uses_llm());
        assert!(PilotMode::Balanced.uses_llm());
        assert!(PilotMode::Conservative.uses_llm());
        assert!(!PilotMode::AlgorithmOnly.uses_llm());
    }

    #[test]
    fn test_budget_config() {
        let config = BudgetConfig::default();
        assert!(config.is_within_budget(1000));
        assert!(!config.is_within_budget(3000));
        assert_eq!(config.remaining_tokens(1500), 500);
    }

    #[test]
    fn test_intervention_config() {
        let config = InterventionConfig::default();

        // Fork threshold
        assert!(!config.should_intervene_at_fork(2));
        assert!(config.should_intervene_at_fork(4));

        // Scores close
        assert!(config.scores_are_close(&[0.5, 0.55, 0.52]));
        assert!(!config.scores_are_close(&[0.3, 0.8]));

        // Low confidence
        assert!(config.is_low_confidence(0.2));
        assert!(!config.is_low_confidence(0.5));
    }

    #[test]
    fn test_pilot_config_presets() {
        let high = PilotConfig::high_quality();
        assert_eq!(high.mode, PilotMode::Aggressive);

        let low = PilotConfig::low_cost();
        assert_eq!(low.mode, PilotMode::Conservative);

        let algo = PilotConfig::algorithm_only();
        assert_eq!(algo.mode, PilotMode::AlgorithmOnly);
    }
}
