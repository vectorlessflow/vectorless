// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Central metrics hub for unified collection.

use std::sync::Arc;

use super::llm::{LlmMetrics, LlmMetricsReport};
use super::pilot::{InterventionPoint, PilotMetrics, PilotMetricsReport};
use super::retrieval::{RetrievalMetrics, RetrievalMetricsReport};
use crate::config::MetricsConfig;

/// Central metrics hub for unified collection.
///
/// Provides a single point for all metrics collection across:
/// - LLM operations (tokens, latency, cost)
/// - Pilot decisions (accuracy, confidence, feedback)
/// - Retrieval operations (paths, scores, cache)
///
/// # Thread Safety
///
/// All metrics use atomic operations and are safe to use from multiple threads.
///
/// # Example
///
/// ```rust
/// use vectorless::metrics::{MetricsHub, MetricsConfig, InterventionPoint};
///
/// let config = MetricsConfig::default();
/// let hub = MetricsHub::new(config);
///
/// // Record LLM call
/// hub.record_llm_call(100, 50, 150, true);
///
/// // Record Pilot decision
/// hub.record_pilot_decision(0.85, InterventionPoint::Fork);
///
/// // Get report
/// let report = hub.generate_report();
/// ```
#[derive(Debug)]
pub struct MetricsHub {
    config: MetricsConfig,
    llm: LlmMetrics,
    pilot: PilotMetrics,
    retrieval: RetrievalMetrics,
}

impl MetricsHub {
    /// Create a new metrics hub.
    pub fn new(config: MetricsConfig) -> Self {
        Self {
            config,
            llm: LlmMetrics::new(),
            pilot: PilotMetrics::new(),
            retrieval: RetrievalMetrics::new(),
        }
    }

    /// Create a new metrics hub with defaults.
    pub fn with_defaults() -> Self {
        Self::new(MetricsConfig::default())
    }

    /// Create an Arc-wrapped metrics hub.
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::with_defaults())
    }

    /// Create an Arc-wrapped metrics hub with config.
    pub fn shared_with_config(config: MetricsConfig) -> Arc<Self> {
        Arc::new(Self::new(config))
    }

    /// Check if metrics are enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the configuration.
    pub fn config(&self) -> &MetricsConfig {
        &self.config
    }

    // ========================================================================
    // LLM Metrics
    // ========================================================================

    /// Record an LLM call.
    pub fn record_llm_call(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        latency_ms: u64,
        success: bool,
    ) {
        if !self.config.enabled || !self.config.llm.track_tokens {
            return;
        }
        self.llm
            .record_call(input_tokens, output_tokens, latency_ms, success, &self.config.llm);
    }

    /// Record an LLM rate limit error.
    pub fn record_llm_rate_limit(&self) {
        if self.config.enabled {
            self.llm.record_rate_limit();
        }
    }

    /// Record an LLM timeout error.
    pub fn record_llm_timeout(&self) {
        if self.config.enabled {
            self.llm.record_timeout();
        }
    }

    /// Record an LLM fallback trigger.
    pub fn record_llm_fallback(&self) {
        if self.config.enabled {
            self.llm.record_fallback();
        }
    }

    /// Get LLM metrics report.
    pub fn llm_report(&self) -> LlmMetricsReport {
        self.llm.generate_report()
    }

    // ========================================================================
    // Pilot Metrics
    // ========================================================================

    /// Record a Pilot decision.
    pub fn record_pilot_decision(&self, confidence: f64, point: InterventionPoint) {
        if !self.config.enabled || !self.config.pilot.track_decisions {
            return;
        }
        self.pilot
            .record_decision(confidence, point, &self.config.pilot);
    }

    /// Record feedback on a Pilot decision.
    pub fn record_pilot_feedback(&self, was_correct: bool) {
        if !self.config.enabled || !self.config.pilot.track_feedback {
            return;
        }
        self.pilot.record_feedback(was_correct, &self.config.pilot);
    }

    /// Record a Pilot LLM call.
    pub fn record_pilot_llm_call(&self) {
        if self.config.enabled {
            self.pilot.record_llm_call();
        }
    }

    /// Record a Pilot intervention.
    pub fn record_pilot_intervention(&self) {
        if self.config.enabled {
            self.pilot.record_intervention();
        }
    }

    /// Record a skipped Pilot intervention.
    pub fn record_pilot_intervention_skipped(&self) {
        if self.config.enabled {
            self.pilot.record_skipped_intervention();
        }
    }

    /// Record Pilot budget exhausted.
    pub fn record_pilot_budget_exhausted(&self) {
        if self.config.enabled {
            self.pilot.record_budget_exhausted();
        }
    }

    /// Record Pilot fallback to algorithm.
    pub fn record_pilot_algorithm_fallback(&self) {
        if self.config.enabled {
            self.pilot.record_algorithm_fallback();
        }
    }

    /// Get Pilot metrics report.
    pub fn pilot_report(&self) -> PilotMetricsReport {
        self.pilot.generate_report()
    }

    // ========================================================================
    // Retrieval Metrics
    // ========================================================================

    /// Record a retrieval query.
    pub fn record_retrieval_query(
        &self,
        iterations: u64,
        nodes_visited: u64,
        latency_ms: u64,
    ) {
        if !self.config.enabled {
            return;
        }
        self.retrieval
            .record_query(iterations, nodes_visited, latency_ms, &self.config.retrieval);
    }

    /// Record a found path.
    pub fn record_retrieval_path(&self, length: u64, score: f64) {
        if !self.config.enabled {
            return;
        }
        self.retrieval
            .record_path(length, score, &self.config.retrieval);
    }

    /// Record a cache hit.
    pub fn record_cache_hit(&self) {
        if !self.config.enabled || !self.config.retrieval.track_cache {
            return;
        }
        self.retrieval.record_cache_hit(&self.config.retrieval);
    }

    /// Record a cache miss.
    pub fn record_cache_miss(&self) {
        if !self.config.enabled || !self.config.retrieval.track_cache {
            return;
        }
        self.retrieval.record_cache_miss(&self.config.retrieval);
    }

    /// Record a backtrack.
    pub fn record_backtrack(&self) {
        if self.config.enabled {
            self.retrieval.record_backtrack();
        }
    }

    /// Record a sufficiency check.
    pub fn record_sufficiency_check(&self, was_sufficient: bool) {
        if self.config.enabled {
            self.retrieval.record_sufficiency_check(was_sufficient);
        }
    }

    /// Get retrieval metrics report.
    pub fn retrieval_report(&self) -> RetrievalMetricsReport {
        self.retrieval.generate_report()
    }

    // ========================================================================
    // General Operations
    // ========================================================================

    /// Reset all metrics.
    pub fn reset(&self) {
        self.llm.reset();
        self.pilot.reset();
        self.retrieval.reset();
    }

    /// Generate a complete report.
    pub fn generate_report(&self) -> MetricsReport {
        MetricsReport {
            llm: self.llm_report(),
            pilot: self.pilot_report(),
            retrieval: self.retrieval_report(),
        }
    }
}

impl Default for MetricsHub {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Complete metrics report.
#[derive(Debug, Clone)]
pub struct MetricsReport {
    /// LLM metrics.
    pub llm: LlmMetricsReport,
    /// Pilot metrics.
    pub pilot: PilotMetricsReport,
    /// Retrieval metrics.
    pub retrieval: RetrievalMetricsReport,
}

impl MetricsReport {
    /// Calculate total estimated cost in USD.
    pub fn total_cost_usd(&self) -> f64 {
        self.llm.estimated_cost_usd
    }

    /// Calculate overall success rate.
    pub fn overall_success_rate(&self) -> f64 {
        let llm_rate = self.llm.success_rate;
        let pilot_rate = if self.pilot.total_decisions > 0 {
            self.pilot.accuracy
        } else {
            1.0
        };
        (llm_rate + pilot_rate) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_hub_recording() {
        let hub = MetricsHub::with_defaults();

        // Record various metrics
        hub.record_llm_call(100, 50, 150, true);
        hub.record_pilot_decision(0.9, InterventionPoint::Fork);
        hub.record_retrieval_query(5, 10, 100);

        let report = hub.generate_report();

        assert_eq!(report.llm.total_calls, 1);
        assert_eq!(report.pilot.total_decisions, 1);
        assert_eq!(report.retrieval.total_queries, 1);
    }

    #[test]
    fn test_metrics_hub_disabled() {
        let config = MetricsConfig::disabled();
        let hub = MetricsHub::new(config);

        hub.record_llm_call(100, 50, 150, true);
        hub.record_pilot_decision(0.9, InterventionPoint::Fork);

        let report = hub.generate_report();

        assert_eq!(report.llm.total_calls, 0);
        assert_eq!(report.pilot.total_decisions, 0);
    }

    #[test]
    fn test_metrics_hub_reset() {
        let hub = MetricsHub::with_defaults();

        hub.record_llm_call(100, 50, 150, true);
        hub.reset();

        let report = hub.generate_report();
        assert_eq!(report.llm.total_calls, 0);
    }
}
