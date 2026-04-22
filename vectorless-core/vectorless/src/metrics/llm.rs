// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM metrics collection.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::LlmMetricsConfig;

/// LLM metrics tracker.
#[derive(Debug, Default)]
pub struct LlmMetrics {
    /// Total number of LLM calls.
    pub total_calls: AtomicU64,
    /// Number of successful calls.
    pub successful_calls: AtomicU64,
    /// Number of failed calls.
    pub failed_calls: AtomicU64,
    /// Total input tokens.
    pub total_input_tokens: AtomicU64,
    /// Total output tokens.
    pub total_output_tokens: AtomicU64,
    /// Total latency in milliseconds.
    pub total_latency_ms: AtomicU64,
    /// Estimated cost in micro-dollars.
    pub estimated_cost_micros: AtomicU64,
    /// Number of rate limit errors.
    pub rate_limit_errors: AtomicU64,
    /// Number of timeout errors.
    pub timeout_errors: AtomicU64,
    /// Number of fallback triggers.
    pub fallback_triggers: AtomicU64,
}

impl LlmMetrics {
    /// Create new LLM metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an LLM call.
    pub fn record_call(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        latency_ms: u64,
        success: bool,
        config: &LlmMetricsConfig,
    ) {
        self.total_calls.fetch_add(1, Ordering::Relaxed);

        if success {
            self.successful_calls.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_calls.fetch_add(1, Ordering::Relaxed);
        }

        if config.track_tokens {
            self.total_input_tokens
                .fetch_add(input_tokens, Ordering::Relaxed);
            self.total_output_tokens
                .fetch_add(output_tokens, Ordering::Relaxed);
        }

        if config.track_latency {
            self.total_latency_ms
                .fetch_add(latency_ms, Ordering::Relaxed);
        }

        if config.track_cost {
            let cost = config.calculate_cost(input_tokens, output_tokens);
            // Store in micro-dollars for precision
            let cost_micros = (cost * 1_000_000.0) as u64;
            self.estimated_cost_micros
                .fetch_add(cost_micros, Ordering::Relaxed);
        }
    }

    /// Record a rate limit error.
    pub fn record_rate_limit(&self) {
        self.rate_limit_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a timeout error.
    pub fn record_timeout(&self) {
        self.timeout_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a fallback trigger.
    pub fn record_fallback(&self) {
        self.fallback_triggers.fetch_add(1, Ordering::Relaxed);
    }

    /// Reset all metrics.
    pub fn reset(&self) {
        self.total_calls.store(0, Ordering::Relaxed);
        self.successful_calls.store(0, Ordering::Relaxed);
        self.failed_calls.store(0, Ordering::Relaxed);
        self.total_input_tokens.store(0, Ordering::Relaxed);
        self.total_output_tokens.store(0, Ordering::Relaxed);
        self.total_latency_ms.store(0, Ordering::Relaxed);
        self.estimated_cost_micros.store(0, Ordering::Relaxed);
        self.rate_limit_errors.store(0, Ordering::Relaxed);
        self.timeout_errors.store(0, Ordering::Relaxed);
        self.fallback_triggers.store(0, Ordering::Relaxed);
    }

    /// Generate a report snapshot.
    pub fn generate_report(&self) -> LlmMetricsReport {
        let total_calls = self.total_calls.load(Ordering::Relaxed);
        let successful = self.successful_calls.load(Ordering::Relaxed);
        let failed = self.failed_calls.load(Ordering::Relaxed);
        let total_latency = self.total_latency_ms.load(Ordering::Relaxed);

        LlmMetricsReport {
            total_calls,
            successful_calls: successful,
            failed_calls: failed,
            success_rate: if total_calls > 0 {
                successful as f64 / total_calls as f64
            } else {
                0.0
            },
            total_input_tokens: self.total_input_tokens.load(Ordering::Relaxed),
            total_output_tokens: self.total_output_tokens.load(Ordering::Relaxed),
            total_tokens: self.total_input_tokens.load(Ordering::Relaxed)
                + self.total_output_tokens.load(Ordering::Relaxed),
            avg_latency_ms: if total_calls > 0 {
                total_latency as f64 / total_calls as f64
            } else {
                0.0
            },
            total_latency_ms: total_latency,
            estimated_cost_usd: self.estimated_cost_micros.load(Ordering::Relaxed) as f64
                / 1_000_000.0,
            rate_limit_errors: self.rate_limit_errors.load(Ordering::Relaxed),
            timeout_errors: self.timeout_errors.load(Ordering::Relaxed),
            fallback_triggers: self.fallback_triggers.load(Ordering::Relaxed),
        }
    }
}

/// LLM metrics report.
#[derive(Debug, Clone)]
pub struct LlmMetricsReport {
    /// Total number of LLM calls.
    pub total_calls: u64,
    /// Number of successful calls.
    pub successful_calls: u64,
    /// Number of failed calls.
    pub failed_calls: u64,
    /// Success rate (0.0 - 1.0).
    pub success_rate: f64,
    /// Total input tokens.
    pub total_input_tokens: u64,
    /// Total output tokens.
    pub total_output_tokens: u64,
    /// Total tokens (input + output).
    pub total_tokens: u64,
    /// Average latency in milliseconds.
    pub avg_latency_ms: f64,
    /// Total latency in milliseconds.
    pub total_latency_ms: u64,
    /// Estimated cost in USD.
    pub estimated_cost_usd: f64,
    /// Number of rate limit errors.
    pub rate_limit_errors: u64,
    /// Number of timeout errors.
    pub timeout_errors: u64,
    /// Number of fallback triggers.
    pub fallback_triggers: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_metrics_recording() {
        let config = LlmMetricsConfig::default();
        let metrics = LlmMetrics::new();

        metrics.record_call(100, 50, 150, true, &config);
        metrics.record_call(200, 100, 300, true, &config);
        metrics.record_call(100, 0, 0, false, &config);

        let report = metrics.generate_report();
        assert_eq!(report.total_calls, 3);
        assert_eq!(report.successful_calls, 2);
        assert_eq!(report.failed_calls, 1);
        assert!((report.success_rate - 0.666666).abs() < 0.01);
        assert_eq!(report.total_input_tokens, 400);
        assert_eq!(report.total_output_tokens, 150);
    }

    #[test]
    fn test_llm_metrics_reset() {
        let config = LlmMetricsConfig::default();
        let metrics = LlmMetrics::new();

        metrics.record_call(100, 50, 150, true, &config);
        metrics.reset();

        let report = metrics.generate_report();
        assert_eq!(report.total_calls, 0);
    }
}
