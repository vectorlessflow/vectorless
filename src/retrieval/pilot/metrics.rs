// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Metrics collector for Pilot operations.
//!
//! Collects performance metrics including:
//! - LLM call statistics (count, success/failure)
//! - Token usage (input, output, total)
//! - Latency tracking (average, p50, p99)
//! - Decision quality metrics

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::decision::InterventionPoint;

/// Snapshot of Pilot metrics at a point in time.
#[derive(Debug, Clone, Default)]
pub struct PilotMetrics {
    // LLM call statistics
    /// Total LLM calls attempted.
    pub total_calls: usize,
    /// Successful LLM calls.
    pub successful_calls: usize,
    /// Failed LLM calls.
    pub failed_calls: usize,
    /// Calls that needed fallback.
    pub fallback_calls: usize,

    // Token statistics
    /// Total input tokens consumed.
    pub total_input_tokens: usize,
    /// Total output tokens generated.
    pub total_output_tokens: usize,
    /// Average tokens per call.
    pub avg_tokens_per_call: f64,

    // Latency statistics
    /// Total time spent in LLM calls (ms).
    pub total_latency_ms: u64,
    /// Average latency per call (ms).
    pub avg_latency_ms: f64,
    /// P50 latency (ms).
    pub p50_latency_ms: u64,
    /// P99 latency (ms).
    pub p99_latency_ms: u64,

    // Intervention statistics
    /// Calls at START point.
    pub start_interventions: usize,
    /// Calls at FORK point.
    pub fork_interventions: usize,
    /// Calls at BACKTRACK point.
    pub backtrack_interventions: usize,
    /// Calls at EVALUATE point.
    pub evaluate_interventions: usize,

    // Quality metrics (require feedback)
    /// LLM decision accuracy (0.0-1.0).
    pub llm_accuracy: Option<f64>,
    /// Retrieval precision (0.0-1.0).
    pub retrieval_precision: Option<f64>,
}

impl PilotMetrics {
    /// Calculate success rate (0.0-1.0).
    pub fn success_rate(&self) -> f64 {
        if self.total_calls == 0 {
            return 0.0;
        }
        self.successful_calls as f64 / self.total_calls as f64
    }

    /// Calculate token utilization.
    pub fn token_utilization(&self, budget: usize) -> f64 {
        if budget == 0 {
            return 0.0;
        }
        let total = self.total_input_tokens + self.total_output_tokens;
        (total as f64 / budget as f64).min(1.0)
    }

    /// Calculate fallback rate (0.0-1.0).
    pub fn fallback_rate(&self) -> f64 {
        if self.total_calls == 0 {
            return 0.0;
        }
        self.fallback_calls as f64 / self.total_calls as f64
    }
}

/// Record of a single LLM call.
#[derive(Debug, Clone)]
pub struct CallRecord {
    /// Intervention point.
    pub point: InterventionPoint,
    /// Input tokens used.
    pub input_tokens: usize,
    /// Output tokens generated.
    pub output_tokens: usize,
    /// Latency in milliseconds.
    pub latency_ms: u64,
    /// Whether the call succeeded.
    pub success: bool,
    /// Whether fallback was used.
    pub used_fallback: bool,
}

/// Latency sample for percentile calculation.
#[derive(Debug, Clone)]
struct LatencySample {
    latency_ms: u64,
}

/// Metrics collector for Pilot operations.
///
/// Thread-safe collector that tracks all Pilot metrics.
/// Uses atomic operations for concurrent access.
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::pilot::MetricsCollector;
///
/// let metrics = MetricsCollector::new();
///
/// // Record a call
/// let start = std::time::Instant::now();
/// // ... make LLM call ...
/// metrics.record_call(InterventionPoint::Fork, 100, 50, start.elapsed(), true, false);
///
/// // Get snapshot
/// let snapshot = metrics.snapshot();
/// println!("Success rate: {:.2}%", snapshot.success_rate() * 100.0);
/// ```
pub struct MetricsCollector {
    // Call counters
    total_calls: AtomicUsize,
    successful_calls: AtomicUsize,
    failed_calls: AtomicUsize,
    fallback_calls: AtomicUsize,

    // Token counters
    total_input_tokens: AtomicUsize,
    total_output_tokens: AtomicUsize,

    // Latency tracking
    total_latency_ms: AtomicU64,
    latency_samples: std::sync::RwLock<Vec<LatencySample>>,

    // Intervention counters
    start_interventions: AtomicUsize,
    fork_interventions: AtomicUsize,
    backtrack_interventions: AtomicUsize,
    evaluate_interventions: AtomicUsize,

    // Quality metrics (set externally)
    llm_accuracy: std::sync::RwLock<Option<f64>>,
    retrieval_precision: std::sync::RwLock<Option<f64>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// Create a new metrics collector.
    pub fn new() -> Self {
        Self {
            total_calls: AtomicUsize::new(0),
            successful_calls: AtomicUsize::new(0),
            failed_calls: AtomicUsize::new(0),
            fallback_calls: AtomicUsize::new(0),
            total_input_tokens: AtomicUsize::new(0),
            total_output_tokens: AtomicUsize::new(0),
            total_latency_ms: AtomicU64::new(0),
            latency_samples: std::sync::RwLock::new(Vec::with_capacity(100)),
            start_interventions: AtomicUsize::new(0),
            fork_interventions: AtomicUsize::new(0),
            backtrack_interventions: AtomicUsize::new(0),
            evaluate_interventions: AtomicUsize::new(0),
            llm_accuracy: std::sync::RwLock::new(None),
            retrieval_precision: std::sync::RwLock::new(None),
        }
    }

    /// Record an LLM call.
    pub fn record_call(
        &self,
        point: InterventionPoint,
        input_tokens: usize,
        output_tokens: usize,
        latency: Duration,
        success: bool,
        used_fallback: bool,
    ) {
        // Update call counters
        self.total_calls.fetch_add(1, Ordering::Relaxed);
        if success {
            self.successful_calls.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_calls.fetch_add(1, Ordering::Relaxed);
        }
        if used_fallback {
            self.fallback_calls.fetch_add(1, Ordering::Relaxed);
        }

        // Update token counters
        self.total_input_tokens.fetch_add(input_tokens, Ordering::Relaxed);
        self.total_output_tokens.fetch_add(output_tokens, Ordering::Relaxed);

        // Update latency
        let latency_ms = latency.as_millis() as u64;
        self.total_latency_ms.fetch_add(latency_ms, Ordering::Relaxed);

        // Store latency sample
        if let Ok(mut samples) = self.latency_samples.write() {
            samples.push(LatencySample { latency_ms });
            // Keep last 1000 samples
            if samples.len() > 1000 {
                samples.remove(0);
            }
        }

        // Update intervention counters
        match point {
            InterventionPoint::Start => {
                self.start_interventions.fetch_add(1, Ordering::Relaxed);
            }
            InterventionPoint::Fork => {
                self.fork_interventions.fetch_add(1, Ordering::Relaxed);
            }
            InterventionPoint::Backtrack => {
                self.backtrack_interventions.fetch_add(1, Ordering::Relaxed);
            }
            InterventionPoint::Evaluate => {
                self.evaluate_interventions.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Record a call using CallRecord.
    pub fn record(&self, record: CallRecord) {
        let latency = Duration::from_millis(record.latency_ms);
        self.record_call(
            record.point,
            record.input_tokens,
            record.output_tokens,
            latency,
            record.success,
            record.used_fallback,
        );
    }

    /// Set LLM accuracy (from external feedback).
    pub fn set_llm_accuracy(&self, accuracy: f64) {
        if let Ok(mut acc) = self.llm_accuracy.write() {
            *acc = Some(accuracy.clamp(0.0, 1.0));
        }
    }

    /// Set retrieval precision (from external feedback).
    pub fn set_retrieval_precision(&self, precision: f64) {
        if let Ok(mut prec) = self.retrieval_precision.write() {
            *prec = Some(precision.clamp(0.0, 1.0));
        }
    }

    /// Get a snapshot of current metrics.
    pub fn snapshot(&self) -> PilotMetrics {
        let total_calls = self.total_calls.load(Ordering::Relaxed);
        let successful_calls = self.successful_calls.load(Ordering::Relaxed);
        let failed_calls = self.failed_calls.load(Ordering::Relaxed);
        let fallback_calls = self.fallback_calls.load(Ordering::Relaxed);
        let total_input_tokens = self.total_input_tokens.load(Ordering::Relaxed);
        let total_output_tokens = self.total_output_tokens.load(Ordering::Relaxed);
        let total_latency_ms = self.total_latency_ms.load(Ordering::Relaxed);

        let avg_tokens_per_call = if total_calls > 0 {
            (total_input_tokens + total_output_tokens) as f64 / total_calls as f64
        } else {
            0.0
        };

        let avg_latency_ms = if total_calls > 0 {
            total_latency_ms as f64 / total_calls as f64
        } else {
            0.0
        };

        // Calculate percentiles from samples
        let (p50_latency_ms, p99_latency_ms) = self.calculate_percentiles();

        PilotMetrics {
            total_calls,
            successful_calls,
            failed_calls,
            fallback_calls,
            total_input_tokens,
            total_output_tokens,
            avg_tokens_per_call,
            total_latency_ms,
            avg_latency_ms,
            p50_latency_ms,
            p99_latency_ms,
            start_interventions: self.start_interventions.load(Ordering::Relaxed),
            fork_interventions: self.fork_interventions.load(Ordering::Relaxed),
            backtrack_interventions: self.backtrack_interventions.load(Ordering::Relaxed),
            evaluate_interventions: self.evaluate_interventions.load(Ordering::Relaxed),
            llm_accuracy: self.llm_accuracy.read().ok().and_then(|v| *v),
            retrieval_precision: self.retrieval_precision.read().ok().and_then(|v| *v),
        }
    }

    /// Calculate p50 and p99 latencies.
    fn calculate_percentiles(&self) -> (u64, u64) {
        if let Ok(samples) = self.latency_samples.read() {
            if samples.is_empty() {
                return (0, 0);
            }

            let mut latencies: Vec<u64> = samples.iter().map(|s| s.latency_ms).collect();
            latencies.sort();

            let p50_idx = (latencies.len() as f64 * 0.50) as usize;
            let p99_idx = (latencies.len() as f64 * 0.99) as usize;

            let p50 = latencies.get(p50_idx).copied().unwrap_or(0);
            let p99 = latencies.get(p99_idx.min(latencies.len() - 1)).copied().unwrap_or(0);

            (p50, p99)
        } else {
            (0, 0)
        }
    }

    /// Reset all metrics for a new query.
    pub fn reset(&self) {
        self.total_calls.store(0, Ordering::Relaxed);
        self.successful_calls.store(0, Ordering::Relaxed);
        self.failed_calls.store(0, Ordering::Relaxed);
        self.fallback_calls.store(0, Ordering::Relaxed);
        self.total_input_tokens.store(0, Ordering::Relaxed);
        self.total_output_tokens.store(0, Ordering::Relaxed);
        self.total_latency_ms.store(0, Ordering::Relaxed);
        self.start_interventions.store(0, Ordering::Relaxed);
        self.fork_interventions.store(0, Ordering::Relaxed);
        self.backtrack_interventions.store(0, Ordering::Relaxed);
        self.evaluate_interventions.store(0, Ordering::Relaxed);

        if let Ok(mut samples) = self.latency_samples.write() {
            samples.clear();
        }
    }

    /// Get total tokens used.
    pub fn total_tokens(&self) -> usize {
        self.total_input_tokens.load(Ordering::Relaxed)
            + self.total_output_tokens.load(Ordering::Relaxed)
    }

    /// Get total calls made.
    pub fn total_calls(&self) -> usize {
        self.total_calls.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_metrics_creation() {
        let metrics = MetricsCollector::new();
        let snapshot = metrics.snapshot();

        assert_eq!(snapshot.total_calls, 0);
        assert_eq!(snapshot.successful_calls, 0);
        assert_eq!(snapshot.failed_calls, 0);
    }

    #[test]
    fn test_record_call() {
        let metrics = MetricsCollector::new();

        metrics.record_call(
            InterventionPoint::Fork,
            100,
            50,
            Duration::from_millis(200),
            true,
            false,
        );

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total_calls, 1);
        assert_eq!(snapshot.successful_calls, 1);
        assert_eq!(snapshot.failed_calls, 0);
        assert_eq!(snapshot.total_input_tokens, 100);
        assert_eq!(snapshot.total_output_tokens, 50);
        assert_eq!(snapshot.fork_interventions, 1);
    }

    #[test]
    fn test_record_failed_call() {
        let metrics = MetricsCollector::new();

        metrics.record_call(
            InterventionPoint::Start,
            100,
            0,
            Duration::from_millis(100),
            false,
            true,
        );

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total_calls, 1);
        assert_eq!(snapshot.successful_calls, 0);
        assert_eq!(snapshot.failed_calls, 1);
        assert_eq!(snapshot.fallback_calls, 1);
        assert_eq!(snapshot.start_interventions, 1);
    }

    #[test]
    fn test_success_rate() {
        let metrics = MetricsCollector::new();

        // No calls
        assert_eq!(metrics.snapshot().success_rate(), 0.0);

        // 3 successful, 1 failed
        metrics.record_call(InterventionPoint::Fork, 0, 0, Duration::ZERO, true, false);
        metrics.record_call(InterventionPoint::Fork, 0, 0, Duration::ZERO, true, false);
        metrics.record_call(InterventionPoint::Fork, 0, 0, Duration::ZERO, true, false);
        metrics.record_call(InterventionPoint::Fork, 0, 0, Duration::ZERO, false, false);

        assert!((metrics.snapshot().success_rate() - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_token_utilization() {
        let metrics = MetricsCollector::new();

        metrics.record_call(InterventionPoint::Fork, 500, 200, Duration::ZERO, true, false);

        let utilization = metrics.snapshot().token_utilization(1000);
        assert!((utilization - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_latency_percentiles() {
        let metrics = MetricsCollector::new();

        // Add 100 samples with increasing latency
        for i in 0..100 {
            metrics.record_call(
                InterventionPoint::Fork,
                0,
                0,
                Duration::from_millis(i as u64 + 1),
                true,
                false,
            );
        }

        let snapshot = metrics.snapshot();

        // P50 should be around 50
        assert!(snapshot.p50_latency_ms >= 40 && snapshot.p50_latency_ms <= 60);

        // P99 should be around 99
        assert!(snapshot.p99_latency_ms >= 90 && snapshot.p99_latency_ms <= 100);
    }

    #[test]
    fn test_reset() {
        let metrics = MetricsCollector::new();

        metrics.record_call(InterventionPoint::Fork, 100, 50, Duration::from_millis(200), true, false);
        assert!(metrics.total_calls() > 0);

        metrics.reset();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total_calls, 0);
        assert_eq!(snapshot.total_input_tokens, 0);
    }

    #[test]
    fn test_quality_metrics() {
        let metrics = MetricsCollector::new();

        metrics.set_llm_accuracy(0.85);
        metrics.set_retrieval_precision(0.92);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.llm_accuracy, Some(0.85));
        assert_eq!(snapshot.retrieval_precision, Some(0.92));
    }

    #[test]
    fn test_quality_metrics_clamping() {
        let metrics = MetricsCollector::new();

        metrics.set_llm_accuracy(1.5);
        metrics.set_retrieval_precision(-0.1);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.llm_accuracy, Some(1.0));
        assert_eq!(snapshot.retrieval_precision, Some(0.0));
    }

    #[test]
    fn test_call_record() {
        let metrics = MetricsCollector::new();

        let record = CallRecord {
            point: InterventionPoint::Backtrack,
            input_tokens: 150,
            output_tokens: 75,
            latency_ms: 300,
            success: true,
            used_fallback: false,
        };

        metrics.record(record);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total_calls, 1);
        assert_eq!(snapshot.backtrack_interventions, 1);
        assert_eq!(snapshot.total_input_tokens, 150);
    }
}
