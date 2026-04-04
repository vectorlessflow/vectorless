// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Budget controller for Pilot LLM calls.
//!
//! Tracks token consumption and call counts to enforce budget limits
//! and control costs during retrieval.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;

use super::config::BudgetConfig;

/// Budget usage statistics.
#[derive(Debug, Clone, Default)]
pub struct BudgetUsage {
    /// Total input tokens used.
    pub input_tokens: usize,
    /// Total output tokens used.
    pub output_tokens: usize,
    /// Total LLM calls made.
    pub calls_made: usize,
    /// Maximum tokens allowed.
    pub max_tokens: usize,
    /// Maximum calls allowed.
    pub max_calls: usize,
}

impl BudgetUsage {
    /// Get total tokens used (input + output).
    pub fn total_tokens(&self) -> usize {
        self.input_tokens + self.output_tokens
    }

    /// Get token utilization (0.0 - 1.0).
    pub fn token_utilization(&self) -> f32 {
        if self.max_tokens == 0 {
            0.0
        } else {
            (self.total_tokens() as f32 / self.max_tokens as f32).min(1.0)
        }
    }

    /// Get call utilization (0.0 - 1.0).
    pub fn call_utilization(&self) -> f32 {
        if self.max_calls == 0 {
            0.0
        } else {
            (self.calls_made as f32 / self.max_calls as f32).min(1.0)
        }
    }

    /// Check if budget is exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.total_tokens() >= self.max_tokens || self.calls_made >= self.max_calls
    }
}

/// Controller for Pilot budget management.
///
/// Tracks token usage and call counts per query, enforcing limits
/// to control costs. Thread-safe for concurrent access.
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::pilot::{BudgetController, BudgetConfig};
///
/// let config = BudgetConfig::default();
/// let budget = BudgetController::new(config);
///
/// // Check if we can make a call
/// if budget.can_call() {
///     // Estimate cost first
///     let estimated = budget.estimate_cost(context);
///     if budget.can_afford(estimated) {
///         // Make the call...
///         budget.record_usage(150, 50, 0);
///     }
/// }
/// ```
pub struct BudgetController {
    config: BudgetConfig,
    /// Total input tokens used.
    input_tokens: AtomicUsize,
    /// Total output tokens used.
    output_tokens: AtomicUsize,
    /// Total calls made.
    calls_made: AtomicUsize,
    /// Calls per level (for level-based limits).
    level_calls: RwLock<HashMap<usize, usize>>,
}

impl BudgetController {
    /// Create a new budget controller with the given config.
    pub fn new(config: BudgetConfig) -> Self {
        Self {
            config,
            input_tokens: AtomicUsize::new(0),
            output_tokens: AtomicUsize::new(0),
            calls_made: AtomicUsize::new(0),
            level_calls: RwLock::new(HashMap::new()),
        }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(BudgetConfig::default())
    }

    /// Check if a new LLM call is allowed.
    ///
    /// Returns `true` if:
    /// - Token budget not exhausted
    /// - Call count not exceeded
    pub fn can_call(&self) -> bool {
        let tokens = self.total_tokens();
        let calls = self.calls_made.load(Ordering::Relaxed);

        tokens < self.config.max_tokens_per_query
            && calls < self.config.max_calls_per_query
    }

    /// Check if a call is allowed at a specific tree level.
    pub fn can_call_at_level(&self, level: usize) -> bool {
        if !self.can_call() {
            return false;
        }

        let level_calls = self.level_calls.read().unwrap();
        let calls = level_calls.get(&level).copied().unwrap_or(0);
        calls < self.config.max_calls_per_level
    }

    /// Estimate token cost for a context string.
    ///
    /// Uses a simple heuristic:
    /// - 1 token ≈ 4 chars (English)
    /// - 1 token ≈ 1.5 chars (Chinese)
    /// - Plus output reserve (100 tokens)
    pub fn estimate_cost(&self, context: &str) -> usize {
        let char_count = context.chars().count();

        // Count Chinese characters
        let chinese_count = context
            .chars()
            .filter(|c| ('\u{4E00}'..='\u{9FFF}').contains(c))
            .count();

        let english_count = char_count - chinese_count;

        // Estimate tokens
        let input_tokens = (chinese_count as f32 / 1.5 + english_count as f32 / 4.0).ceil() as usize;

        // Add output reserve
        input_tokens + 100
    }

    /// Check if we can afford an estimated cost.
    pub fn can_afford(&self, estimated_cost: usize) -> bool {
        let remaining = self.remaining_tokens();

        estimated_cost <= remaining
            && estimated_cost <= self.config.max_tokens_per_call
    }

    /// Get remaining token budget.
    pub fn remaining_tokens(&self) -> usize {
        self.config
            .max_tokens_per_query
            .saturating_sub(self.total_tokens())
    }

    /// Get remaining call budget.
    pub fn remaining_calls(&self) -> usize {
        self.config
            .max_calls_per_query
            .saturating_sub(self.calls_made.load(Ordering::Relaxed))
    }

    /// Record token usage after an LLM call.
    ///
    /// # Arguments
    ///
    /// * `input_tokens` - Tokens in the prompt
    /// * `output_tokens` - Tokens in the response
    /// * `level` - Tree level where call was made
    pub fn record_usage(&self, input_tokens: usize, output_tokens: usize, level: usize) {
        self.input_tokens.fetch_add(input_tokens, Ordering::Relaxed);
        self.output_tokens.fetch_add(output_tokens, Ordering::Relaxed);
        self.calls_made.fetch_add(1, Ordering::Relaxed);

        // Track level calls
        {
            let mut level_calls = self.level_calls.write().unwrap();
            *level_calls.entry(level).or_insert(0) += 1;
        }
    }

    /// Get total tokens used.
    pub fn total_tokens(&self) -> usize {
        self.input_tokens.load(Ordering::Relaxed)
            + self.output_tokens.load(Ordering::Relaxed)
    }

    /// Get current usage statistics.
    pub fn usage(&self) -> BudgetUsage {
        BudgetUsage {
            input_tokens: self.input_tokens.load(Ordering::Relaxed),
            output_tokens: self.output_tokens.load(Ordering::Relaxed),
            calls_made: self.calls_made.load(Ordering::Relaxed),
            max_tokens: self.config.max_tokens_per_query,
            max_calls: self.config.max_calls_per_query,
        }
    }

    /// Get calls made at a specific level.
    pub fn calls_at_level(&self, level: usize) -> usize {
        let level_calls = self.level_calls.read().unwrap();
        level_calls.get(&level).copied().unwrap_or(0)
    }

    /// Reset budget state for a new query.
    pub fn reset(&self) {
        self.input_tokens.store(0, Ordering::Relaxed);
        self.output_tokens.store(0, Ordering::Relaxed);
        self.calls_made.store(0, Ordering::Relaxed);
        self.level_calls.write().unwrap().clear();
    }

    /// Get the configuration.
    pub fn config(&self) -> &BudgetConfig {
        &self.config
    }

    /// Check if hard limit is enforced.
    pub fn is_hard_limit(&self) -> bool {
        self.config.hard_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_controller_new() {
        let config = BudgetConfig::default();
        let budget = BudgetController::new(config);

        assert!(budget.can_call());
        assert_eq!(budget.remaining_calls(), config.max_calls_per_query);
    }

    #[test]
    fn test_budget_can_call() {
        let config = BudgetConfig {
            max_tokens_per_query: 100,
            max_calls_per_query: 2,
            ..Default::default()
        };
        let budget = BudgetController::new(config);

        assert!(budget.can_call());

        budget.record_usage(50, 30, 0);
        assert!(budget.can_call()); // 80 tokens, 1 call

        budget.record_usage(50, 30, 0);
        assert!(!budget.can_call()); // 160 tokens, 2 calls - exceeded
    }

    #[test]
    fn test_budget_level_limit() {
        let config = BudgetConfig {
            max_calls_per_query: 10,
            max_calls_per_level: 2,
            ..Default::default()
        };
        let budget = BudgetController::new(config);

        assert!(budget.can_call_at_level(0));

        budget.record_usage(10, 10, 0);
        budget.record_usage(10, 10, 0);
        assert!(!budget.can_call_at_level(0)); // 2 calls at level 0
        assert!(budget.can_call_at_level(1)); // Can still call at level 1
    }

    #[test]
    fn test_budget_estimate_cost() {
        let budget = BudgetController::with_defaults();

        // English text
        let english = "Hello world this is a test";
        let cost = budget.estimate_cost(english);
        assert!(cost > 0 && cost < 50);

        // Chinese text
        let chinese = "这是一个测试";
        let cost_chinese = budget.estimate_cost(chinese);
        assert!(cost_chinese > cost); // Chinese uses more tokens
    }

    #[test]
    fn test_budget_can_afford() {
        let config = BudgetConfig {
            max_tokens_per_query: 200,
            max_tokens_per_call: 100,
            ..Default::default()
        };
        let budget = BudgetController::new(config);

        assert!(budget.can_afford(50));
        assert!(budget.can_afford(100));
        assert!(!budget.can_afford(150)); // Exceeds max_tokens_per_call

        budget.record_usage(100, 50, 0); // 150 tokens used
        assert!(budget.can_afford(50)); // 50 remaining
        assert!(!budget.can_afford(60)); // Only 50 remaining
    }

    #[test]
    fn test_budget_reset() {
        let budget = BudgetController::with_defaults();

        budget.record_usage(100, 50, 0);
        assert_eq!(budget.total_tokens(), 150);
        assert_eq!(budget.calls_made.load(Ordering::Relaxed), 1);

        budget.reset();
        assert_eq!(budget.total_tokens(), 0);
        assert_eq!(budget.calls_made.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_budget_usage_stats() {
        let budget = BudgetController::with_defaults();

        budget.record_usage(100, 50, 0);
        let usage = budget.usage();

        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.calls_made, 1);
        assert_eq!(usage.total_tokens(), 150);
    }
}
