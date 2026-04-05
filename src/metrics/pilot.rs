// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pilot metrics collection.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::PilotMetricsConfig;

/// Intervention point type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterventionPoint {
    /// At search start.
    Start,
    /// At a fork (multiple candidates).
    Fork,
    /// During backtracking.
    Backtrack,
    /// Evaluating content sufficiency.
    Evaluate,
}

/// Helper to store f64 as u64 bits for atomic operations.
fn f64_to_u64_bits(v: f64) -> u64 {
    v.to_bits()
}

/// Helper to convert u64 bits back to f64.
fn u64_bits_to_f64(v: u64) -> f64 {
    f64::from_bits(v)
}

/// Pilot metrics tracker.
#[derive(Debug, Default)]
pub struct PilotMetrics {
    /// Total number of Pilot decisions.
    pub total_decisions: AtomicU64,
    /// Number of start guidance calls.
    pub start_guidance_calls: AtomicU64,
    /// Number of fork decisions.
    pub fork_decisions: AtomicU64,
    /// Number of backtrack guidance calls.
    pub backtrack_calls: AtomicU64,
    /// Number of evaluate calls.
    pub evaluate_calls: AtomicU64,
    /// Number of correct decisions (based on feedback).
    pub correct_decisions: AtomicU64,
    /// Number of incorrect decisions (based on feedback).
    pub incorrect_decisions: AtomicU64,
    /// Sum of confidence values stored as u64 bits (for atomic ops).
    /// We store the sum scaled by 1,000,000 to maintain precision.
    pub confidence_sum_scaled: AtomicU64,
    /// Number of confidence samples.
    pub confidence_count: AtomicU64,
    /// Number of LLM calls made by Pilot.
    pub llm_calls: AtomicU64,
    /// Number of times Pilot intervened.
    pub interventions: AtomicU64,
    /// Number of times Pilot skipped intervention (algorithm was confident).
    pub skipped_interventions: AtomicU64,
    /// Number of budget exhausted events.
    pub budget_exhausted: AtomicU64,
    /// Number of fallback to algorithm.
    pub algorithm_fallbacks: AtomicU64,
}

impl PilotMetrics {
    /// Create new Pilot metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a Pilot decision.
    pub fn record_decision(&self, confidence: f64, point: InterventionPoint, config: &PilotMetricsConfig) {
        if !config.track_decisions {
            return;
        }

        self.total_decisions.fetch_add(1, Ordering::Relaxed);

        match point {
            InterventionPoint::Start => {
                self.start_guidance_calls.fetch_add(1, Ordering::Relaxed);
            }
            InterventionPoint::Fork => {
                self.fork_decisions.fetch_add(1, Ordering::Relaxed);
            }
            InterventionPoint::Backtrack => {
                self.backtrack_calls.fetch_add(1, Ordering::Relaxed);
            }
            InterventionPoint::Evaluate => {
                self.evaluate_calls.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Update average confidence (store as scaled integer for atomic operations)
        let scaled_confidence = (confidence * 1_000_000.0) as u64;
        self.confidence_sum_scaled.fetch_add(scaled_confidence, Ordering::Relaxed);
        self.confidence_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record feedback on a decision.
    pub fn record_feedback(&self, was_correct: bool, config: &PilotMetricsConfig) {
        if !config.track_feedback {
            return;
        }

        if was_correct {
            self.correct_decisions.fetch_add(1, Ordering::Relaxed);
        } else {
            self.incorrect_decisions.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record an LLM call made by Pilot.
    pub fn record_llm_call(&self) {
        self.llm_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an intervention.
    pub fn record_intervention(&self) {
        self.interventions.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a skipped intervention.
    pub fn record_skipped_intervention(&self) {
        self.skipped_interventions.fetch_add(1, Ordering::Relaxed);
    }

    /// Record budget exhausted.
    pub fn record_budget_exhausted(&self) {
        self.budget_exhausted.fetch_add(1, Ordering::Relaxed);
    }

    /// Record algorithm fallback.
    pub fn record_algorithm_fallback(&self) {
        self.algorithm_fallbacks.fetch_add(1, Ordering::Relaxed);
    }

    /// Reset all metrics.
    pub fn reset(&self) {
        self.total_decisions.store(0, Ordering::Relaxed);
        self.start_guidance_calls.store(0, Ordering::Relaxed);
        self.fork_decisions.store(0, Ordering::Relaxed);
        self.backtrack_calls.store(0, Ordering::Relaxed);
        self.evaluate_calls.store(0, Ordering::Relaxed);
        self.correct_decisions.store(0, Ordering::Relaxed);
        self.incorrect_decisions.store(0, Ordering::Relaxed);
        self.confidence_sum_scaled.store(0, Ordering::Relaxed);
        self.confidence_count.store(0, Ordering::Relaxed);
        self.llm_calls.store(0, Ordering::Relaxed);
        self.interventions.store(0, Ordering::Relaxed);
        self.skipped_interventions.store(0, Ordering::Relaxed);
        self.budget_exhausted.store(0, Ordering::Relaxed);
        self.algorithm_fallbacks.store(0, Ordering::Relaxed);
    }

    /// Generate a report snapshot.
    pub fn generate_report(&self) -> PilotMetricsReport {
        let total_decisions = self.total_decisions.load(Ordering::Relaxed);
        let correct = self.correct_decisions.load(Ordering::Relaxed);
        let total_feedback = correct + self.incorrect_decisions.load(Ordering::Relaxed);
        let confidence_count = self.confidence_count.load(Ordering::Relaxed);
        let confidence_sum_scaled = self.confidence_sum_scaled.load(Ordering::Relaxed);

        PilotMetricsReport {
            total_decisions,
            start_guidance_calls: self.start_guidance_calls.load(Ordering::Relaxed),
            fork_decisions: self.fork_decisions.load(Ordering::Relaxed),
            backtrack_calls: self.backtrack_calls.load(Ordering::Relaxed),
            evaluate_calls: self.evaluate_calls.load(Ordering::Relaxed),
            accuracy: if total_feedback > 0 {
                correct as f64 / total_feedback as f64
            } else {
                0.0
            },
            correct_decisions: correct,
            incorrect_decisions: self.incorrect_decisions.load(Ordering::Relaxed),
            avg_confidence: if confidence_count > 0 {
                (confidence_sum_scaled as f64 / 1_000_000.0) / confidence_count as f64
            } else {
                0.0
            },
            llm_calls: self.llm_calls.load(Ordering::Relaxed),
            interventions: self.interventions.load(Ordering::Relaxed),
            skipped_interventions: self.skipped_interventions.load(Ordering::Relaxed),
            budget_exhausted: self.budget_exhausted.load(Ordering::Relaxed),
            algorithm_fallbacks: self.algorithm_fallbacks.load(Ordering::Relaxed),
        }
    }
}

/// Pilot metrics report.
#[derive(Debug, Clone)]
pub struct PilotMetricsReport {
    /// Total number of decisions.
    pub total_decisions: u64,
    /// Number of start guidance calls.
    pub start_guidance_calls: u64,
    /// Number of fork decisions.
    pub fork_decisions: u64,
    /// Number of backtrack calls.
    pub backtrack_calls: u64,
    /// Number of evaluate calls.
    pub evaluate_calls: u64,
    /// Decision accuracy based on feedback.
    pub accuracy: f64,
    /// Number of correct decisions.
    pub correct_decisions: u64,
    /// Number of incorrect decisions.
    pub incorrect_decisions: u64,
    /// Average confidence across all decisions.
    pub avg_confidence: f64,
    /// Number of LLM calls made by Pilot.
    pub llm_calls: u64,
    /// Number of interventions.
    pub interventions: u64,
    /// Number of skipped interventions.
    pub skipped_interventions: u64,
    /// Number of budget exhausted events.
    pub budget_exhausted: u64,
    /// Number of algorithm fallbacks.
    pub algorithm_fallbacks: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pilot_metrics_recording() {
        let config = PilotMetricsConfig::default();
        let metrics = PilotMetrics::new();

        metrics.record_decision(0.9, InterventionPoint::Start, &config);
        metrics.record_decision(0.8, InterventionPoint::Fork, &config);
        metrics.record_decision(0.7, InterventionPoint::Fork, &config);

        metrics.record_feedback(true, &config);
        metrics.record_feedback(false, &config);

        let report = metrics.generate_report();
        assert_eq!(report.total_decisions, 3);
        assert_eq!(report.fork_decisions, 2);
        assert!((report.accuracy - 0.5).abs() < 0.01);
        assert!((report.avg_confidence - 0.8).abs() < 0.01);
    }
}
