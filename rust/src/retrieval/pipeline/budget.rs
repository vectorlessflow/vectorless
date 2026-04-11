// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Adaptive token budget controller for the retrieval pipeline.
//!
//! Unlike the Pilot-level [`BudgetController`](crate::retrieval::pilot::BudgetController)
//! which only tracks Pilot LLM calls, this controller tracks the **entire pipeline's**
//! token consumption across all stages and provides dynamic budget allocation decisions.
//!
//! # Design
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │          RetrievalBudgetController                │
//! │                                                   │
//! │  total_budget ────────────────────────┬────────── │
//! │  consumed (from all stages)           │ remaining │
//! │                                       │           │
//! │  Plan stage: initial allocation       │           │
//! │  Search stage: check before iteration │           │
//! │  Evaluate stage: report & decide      │           │
//! │  Graceful degradation when low        │           │
//! └──────────────────────────────────────────────────┘
//! ```

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Status of the budget for stage-level decision making.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetStatus {
    /// Plenty of budget remaining, proceed normally.
    Healthy,
    /// Budget is getting low, consider cheaper strategies.
    Constrained,
    /// Budget is exhausted, stop LLM calls and return best results.
    Exhausted,
}

impl BudgetStatus {
    /// Whether LLM calls should still be made.
    pub fn allow_llm(self) -> bool {
        matches!(self, Self::Healthy | Self::Constrained)
    }

    /// Whether the pipeline should stop iterating and return current results.
    pub fn should_stop(self) -> bool {
        self == Self::Exhausted
    }
}

/// Adaptive budget controller for the retrieval pipeline.
///
/// Tracks token consumption across all stages (Plan, Search, Evaluate)
/// and provides budget-aware decisions for dynamic strategy adjustment.
///
/// # Example
///
/// ```rust,ignore
/// let budget = RetrievalBudgetController::new(4000);
///
/// // In Search stage: check before starting an iteration
/// if budget.status().should_stop() {
///     return StageOutcome::complete(); // graceful degradation
/// }
///
/// // After LLM call: record consumption
/// budget.record_tokens(350);
///
/// // In Evaluate: decide based on remaining budget
/// if budget.status() == BudgetStatus::Constrained {
///     // Use cheaper sufficiency check
/// }
/// ```
pub struct RetrievalBudgetController {
    /// Total token budget for this retrieval operation.
    total_budget: usize,
    /// Tokens consumed so far (atomic for thread safety).
    consumed: AtomicUsize,
    /// Whether budget exhaustion has been signaled to the pipeline.
    exhaustion_signaled: AtomicBool,
    /// Threshold ratio for "constrained" status (e.g. 0.7 = warn at 70% used).
    constrain_threshold: f32,
}

// Manual Clone because AtomicUsize/AtomicBool don't impl Clone.
impl Clone for RetrievalBudgetController {
    fn clone(&self) -> Self {
        Self {
            total_budget: self.total_budget,
            consumed: AtomicUsize::new(self.consumed.load(Ordering::Relaxed)),
            exhaustion_signaled: AtomicBool::new(
                self.exhaustion_signaled.load(Ordering::Relaxed),
            ),
            constrain_threshold: self.constrain_threshold,
        }
    }
}

impl RetrievalBudgetController {
    /// Create a new budget controller with the given total token budget.
    pub fn new(total_budget: usize) -> Self {
        Self {
            total_budget,
            consumed: AtomicUsize::new(0),
            exhaustion_signaled: AtomicBool::new(false),
            constrain_threshold: 0.7,
        }
    }

    /// Create with a custom constrain threshold (0.0 - 1.0).
    ///
    /// When consumption exceeds `total_budget * threshold`, status becomes Constrained.
    pub fn with_constrain_threshold(mut self, threshold: f32) -> Self {
        self.constrain_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Get the current budget status.
    pub fn status(&self) -> BudgetStatus {
        if self.exhaustion_signaled.load(Ordering::Relaxed) {
            return BudgetStatus::Exhausted;
        }

        let consumed = self.consumed.load(Ordering::Relaxed);
        if consumed >= self.total_budget {
            self.exhaustion_signaled.store(true, Ordering::Relaxed);
            return BudgetStatus::Exhausted;
        }

        let utilization = consumed as f32 / self.total_budget as f32;
        if utilization >= self.constrain_threshold {
            BudgetStatus::Constrained
        } else {
            BudgetStatus::Healthy
        }
    }

    /// Record tokens consumed by any stage.
    pub fn record_tokens(&self, tokens: usize) {
        self.consumed.fetch_add(tokens, Ordering::Relaxed);
    }

    /// Get total tokens consumed so far.
    pub fn consumed(&self) -> usize {
        self.consumed.load(Ordering::Relaxed)
    }

    /// Get remaining token budget.
    pub fn remaining(&self) -> usize {
        self.total_budget.saturating_sub(self.consumed.load(Ordering::Relaxed))
    }

    /// Get total budget.
    pub fn total_budget(&self) -> usize {
        self.total_budget
    }

    /// Get utilization ratio (0.0 - 1.0).
    pub fn utilization(&self) -> f32 {
        if self.total_budget == 0 {
            0.0
        } else {
            (self.consumed.load(Ordering::Relaxed) as f32 / self.total_budget as f32).min(1.0)
        }
    }

    /// Signal that budget is exhausted (e.g. external trigger).
    pub fn signal_exhausted(&self) {
        self.exhaustion_signaled.store(true, Ordering::Relaxed);
    }

    /// Whether budget exhaustion has been signaled.
    pub fn is_exhausted(&self) -> bool {
        self.exhaustion_signaled.load(Ordering::Relaxed)
            || self.consumed.load(Ordering::Relaxed) >= self.total_budget
    }

    /// Reset for a new query.
    pub fn reset(&self) {
        self.consumed.store(0, Ordering::Relaxed);
        self.exhaustion_signaled.store(false, Ordering::Relaxed);
    }

    /// Suggest a search strategy based on budget status and query complexity.
    ///
    /// Returns the recommended beam width for the next search iteration.
    pub fn suggested_beam_width(&self, current_beam: usize, iteration: usize) -> usize {
        match self.status() {
            BudgetStatus::Healthy => {
                // Full power, maybe even increase beam for complex queries
                current_beam
            }
            BudgetStatus::Constrained => {
                // Reduce beam to save tokens
                let reduced = if iteration <= 1 { current_beam } else { (current_beam / 2).max(1) };
                reduced
            }
            BudgetStatus::Exhausted => {
                // No more search iterations worth doing
                0
            }
        }
    }

    /// Whether another search iteration is worthwhile given budget and confidence.
    pub fn should_continue_search(&self, current_confidence: f32, iteration: usize) -> bool {
        if self.is_exhausted() {
            return false;
        }
        // Don't continue if confidence is already good
        if current_confidence > 0.8 && iteration >= 1 {
            return false;
        }
        // Don't continue if budget is constrained and we have some results
        if self.status() == BudgetStatus::Constrained && current_confidence > 0.4 {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_healthy() {
        let budget = RetrievalBudgetController::new(1000);
        assert_eq!(budget.status(), BudgetStatus::Healthy);
        assert!(!budget.is_exhausted());
        assert_eq!(budget.remaining(), 1000);
    }

    #[test]
    fn test_budget_constrained() {
        let budget = RetrievalBudgetController::new(1000);
        budget.record_tokens(750); // 75% used, above 70% threshold
        assert_eq!(budget.status(), BudgetStatus::Constrained);
        assert!(budget.status().allow_llm());
    }

    #[test]
    fn test_budget_exhausted() {
        let budget = RetrievalBudgetController::new(1000);
        budget.record_tokens(1000);
        assert_eq!(budget.status(), BudgetStatus::Exhausted);
        assert!(budget.status().should_stop());
        assert!(!budget.status().allow_llm());
    }

    #[test]
    fn test_budget_exhausted_over() {
        let budget = RetrievalBudgetController::new(1000);
        budget.record_tokens(1500);
        assert_eq!(budget.status(), BudgetStatus::Exhausted);
    }

    #[test]
    fn test_budget_signal_exhausted() {
        let budget = RetrievalBudgetController::new(1000);
        budget.signal_exhausted();
        assert_eq!(budget.status(), BudgetStatus::Exhausted);
        assert_eq!(budget.consumed(), 0); // No tokens actually consumed
    }

    #[test]
    fn test_budget_reset() {
        let budget = RetrievalBudgetController::new(1000);
        budget.record_tokens(800);
        assert_eq!(budget.status(), BudgetStatus::Constrained);
        budget.reset();
        assert_eq!(budget.status(), BudgetStatus::Healthy);
        assert_eq!(budget.consumed(), 0);
    }

    #[test]
    fn test_suggested_beam_width() {
        let budget = RetrievalBudgetController::new(1000);
        // Healthy: keep current beam
        assert_eq!(budget.suggested_beam_width(4, 0), 4);

        // Constrained: first iteration keeps beam, later reduces
        budget.record_tokens(750);
        assert_eq!(budget.suggested_beam_width(4, 0), 4);
        assert_eq!(budget.suggested_beam_width(4, 2), 2);

        // Exhausted: zero
        budget.record_tokens(300);
        assert_eq!(budget.suggested_beam_width(4, 0), 0);
    }

    #[test]
    fn test_should_continue_search() {
        let budget = RetrievalBudgetController::new(1000);

        // Fresh, low confidence: continue
        assert!(budget.should_continue_search(0.2, 0));

        // High confidence after 1 iteration: stop
        assert!(!budget.should_continue_search(0.9, 1));

        // Medium confidence, healthy budget: continue
        assert!(budget.should_continue_search(0.5, 1));

        // Constrained, decent confidence: stop
        budget.record_tokens(750);
        assert!(!budget.should_continue_search(0.5, 2));

        // Constrained, low confidence: continue
        assert!(budget.should_continue_search(0.2, 2));
    }

    #[test]
    fn test_utilization() {
        let budget = RetrievalBudgetController::new(1000);
        assert!((budget.utilization() - 0.0).abs() < 0.01);

        budget.record_tokens(500);
        assert!((budget.utilization() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_custom_constrain_threshold() {
        let budget = RetrievalBudgetController::new(1000).with_constrain_threshold(0.5);
        budget.record_tokens(500);
        assert_eq!(budget.status(), BudgetStatus::Constrained);
    }
}
