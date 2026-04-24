// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Failure policies for pipeline passes.
//!
//! This module provides configurable failure handling for compile pipeline passes.
//!
//! # Policies
//!
//! - **Fail** - Stop the entire pipeline on pass failure (default for required passes)
//! - **Skip** - Skip the failed pass and continue the pipeline
//! - **Retry** - Retry the pass with exponential backoff before failing
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::index::pipeline::{FailurePolicy, StageRetryConfig};
//!
//! // Simple skip policy
//! let policy = FailurePolicy::skip();
//!
//! // Retry with custom config
//! let policy = FailurePolicy::retry_with(
//!     StageRetryConfig::new()
//!         .with_max_attempts(3)
//!         .with_initial_delay(Duration::from_millis(500))
//! );
//! ```

use std::time::Duration;

/// Retry configuration for stage execution.
#[derive(Debug, Clone)]
pub struct StageRetryConfig {
    /// Maximum number of attempts (including initial).
    pub max_attempts: usize,
    /// Initial delay before first retry.
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Exponential backoff multiplier.
    pub multiplier: f64,
}

impl Default for StageRetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
        }
    }
}

impl StageRetryConfig {
    /// Create a new retry config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum number of attempts.
    pub fn with_max_attempts(mut self, n: usize) -> Self {
        self.max_attempts = n.max(1);
        self
    }

    /// Set initial delay before first retry.
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Set maximum delay between retries.
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Set exponential backoff multiplier.
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }

    /// Calculate delay for a given attempt (0-indexed).
    ///
    /// Uses exponential backoff: `initial_delay * multiplier^attempt`
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        let delay_ms =
            (self.initial_delay.as_millis() as f64) * self.multiplier.powi(attempt as i32);
        let capped_ms = delay_ms.min(self.max_delay.as_millis() as f64);
        Duration::from_millis(capped_ms as u64)
    }
}

/// Policy for handling stage failures.
#[derive(Debug, Clone)]
pub enum FailurePolicy {
    /// Fail the entire pipeline on error (default for required stages).
    Fail,

    /// Skip this stage on failure, continue pipeline.
    /// The stage result will record the failure but execution continues.
    Skip,

    /// Retry with specified configuration before failing.
    /// If all retries fail, the pipeline behavior depends on `allows_continuation`.
    Retry(StageRetryConfig),
}

impl Default for FailurePolicy {
    fn default() -> Self {
        Self::Fail
    }
}

impl FailurePolicy {
    /// Create a Fail policy.
    pub fn fail() -> Self {
        Self::Fail
    }

    /// Create a Skip policy.
    pub fn skip() -> Self {
        Self::Skip
    }

    /// Create a Retry policy with default configuration.
    pub fn retry() -> Self {
        Self::Retry(StageRetryConfig::default())
    }

    /// Create a Retry policy with custom configuration.
    pub fn retry_with(config: StageRetryConfig) -> Self {
        Self::Retry(config)
    }

    /// Check if pipeline can continue after failure with this policy.
    ///
    /// - `Fail`: No, stops pipeline
    /// - `Skip`: Yes, continues
    /// - `Retry`: No (if all retries exhausted, it's treated as failure)
    pub fn allows_continuation(&self) -> bool {
        matches!(self, Self::Skip)
    }

    /// Check if this policy involves retry attempts.
    pub fn has_retry(&self) -> bool {
        matches!(self, Self::Retry(_))
    }

    /// Get retry config if this is a Retry policy.
    pub fn retry_config(&self) -> Option<&StageRetryConfig> {
        match self {
            Self::Retry(config) => Some(config),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_retry_config() {
        let config = StageRetryConfig::default();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(100));
        assert_eq!(config.max_delay, Duration::from_secs(10));
    }

    #[test]
    fn test_retry_config_builder() {
        let config = StageRetryConfig::new()
            .with_max_attempts(5)
            .with_initial_delay(Duration::from_millis(200))
            .with_max_delay(Duration::from_secs(30));

        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_delay, Duration::from_millis(200));
        assert_eq!(config.max_delay, Duration::from_secs(30));
    }

    #[test]
    fn test_delay_for_attempt() {
        let config = StageRetryConfig::new()
            .with_initial_delay(Duration::from_millis(100))
            .with_multiplier(2.0);

        assert_eq!(config.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(config.delay_for_attempt(2), Duration::from_millis(400));
    }

    #[test]
    fn test_delay_respects_max() {
        let config = StageRetryConfig::new()
            .with_initial_delay(Duration::from_secs(1))
            .with_max_delay(Duration::from_secs(5))
            .with_multiplier(10.0);

        assert_eq!(config.delay_for_attempt(0), Duration::from_secs(1));
        assert_eq!(config.delay_for_attempt(1), Duration::from_secs(5)); // capped
        assert_eq!(config.delay_for_attempt(2), Duration::from_secs(5)); // capped
    }

    #[test]
    fn test_failure_policy_constructors() {
        assert!(matches!(FailurePolicy::fail(), FailurePolicy::Fail));
        assert!(matches!(FailurePolicy::skip(), FailurePolicy::Skip));
        assert!(matches!(FailurePolicy::retry(), FailurePolicy::Retry(_)));
    }

    #[test]
    fn test_allows_continuation() {
        assert!(!FailurePolicy::fail().allows_continuation());
        assert!(FailurePolicy::skip().allows_continuation());
        assert!(!FailurePolicy::retry().allows_continuation());
    }
}
