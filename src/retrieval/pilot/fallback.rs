// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Fallback manager for Pilot LLM calls.
//!
//! Implements layered fallback strategy:
//! 1. Normal LLM call
//! 2. Retry with exponential backoff
//! 3. Simplified context (reduce tokens)
//! 4. Algorithm-only mode (no LLM)

use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::time::Duration;
use tracing::{debug, warn};

/// Fallback level indicating current degradation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackLevel {
    /// Normal operation - LLM calls working.
    Normal = 0,
    /// Retrying - transient failures, using backoff.
    Retry = 1,
    /// Simplified - using reduced context.
    Simplified = 2,
    /// Algorithm only - LLM unavailable.
    AlgorithmOnly = 3,
}

impl Default for FallbackLevel {
    fn default() -> Self {
        Self::Normal
    }
}

impl From<u8> for FallbackLevel {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Normal,
            1 => Self::Retry,
            2 => Self::Simplified,
            _ => Self::AlgorithmOnly,
        }
    }
}

/// Configuration for fallback behavior.
#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// Maximum retry attempts before escalating.
    pub max_retries: usize,
    /// Initial delay for exponential backoff (ms).
    pub initial_delay_ms: u64,
    /// Maximum delay for exponential backoff (ms).
    pub max_delay_ms: u64,
    /// Multiplier for exponential backoff.
    pub backoff_multiplier: f64,
    /// Consecutive failures before escalating level.
    pub failures_before_escalate: usize,
    /// Consecutive successes before de-escalating level.
    pub successes_before_deescalate: usize,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 10000,
            backoff_multiplier: 2.0,
            failures_before_escalate: 3,
            successes_before_deescalate: 2,
        }
    }
}

/// Errors that can trigger fallback.
#[derive(Debug, Clone, thiserror::Error)]
pub enum FallbackError {
    /// Network/timeout error (retryable).
    #[error("Network error: {0}")]
    Network(String),
    /// Rate limit error (retryable with backoff).
    #[error("Rate limited")]
    RateLimited,
    /// Token limit exceeded (need simplified context).
    #[error("Token limit exceeded")]
    TokenLimitExceeded,
    /// LLM service unavailable (use algorithm).
    #[error("LLM unavailable: {0}")]
    Unavailable(String),
    /// Parsing error (may use default).
    #[error("Response parsing failed: {0}")]
    ParseError(String),
    /// All fallbacks exhausted.
    #[error("All fallback strategies exhausted")]
    Exhausted,
}

impl FallbackError {
    /// Check if this error should trigger a retry.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Network(_) | Self::RateLimited)
    }

    /// Check if this error suggests using simplified context.
    pub fn needs_simplification(&self) -> bool {
        matches!(self, Self::TokenLimitExceeded)
    }

    /// Check if this error requires algorithm fallback.
    pub fn needs_algorithm_fallback(&self) -> bool {
        matches!(self, Self::Unavailable(_) | Self::Exhausted)
    }
}

/// Statistics for fallback operations.
#[derive(Debug, Clone, Default)]
pub struct FallbackStats {
    /// Total operations attempted.
    pub total_attempts: usize,
    /// Successful operations (no fallback needed).
    pub successful: usize,
    /// Operations that needed retry.
    pub retried: usize,
    /// Operations that needed simplified context.
    pub simplified: usize,
    /// Operations that fell back to algorithm.
    pub algorithm_fallbacks: usize,
    /// Current fallback level.
    pub current_level: FallbackLevel,
}

/// Manager for handling LLM call failures with layered fallback.
///
/// Implements a 4-level fallback strategy:
/// 1. Normal: Direct LLM calls
/// 2. Retry: Exponential backoff retry
/// 3. Simplified: Reduced context to fit token limits
/// 4. Algorithm: Pure algorithm mode, no LLM
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::pilot::FallbackManager;
///
/// let manager = FallbackManager::new(FallbackConfig::default());
///
/// // Check current level
/// if manager.current_level() == FallbackLevel::Normal {
///     // Make LLM call
/// }
///
/// // Record failure
/// manager.record_failure(&error);
/// ```
pub struct FallbackManager {
    config: FallbackConfig,
    /// Current fallback level.
    current_level: AtomicU8,
    /// Consecutive failures at current level.
    consecutive_failures: AtomicUsize,
    /// Consecutive successes at current level.
    consecutive_successes: AtomicUsize,
    /// Total retry attempts in current session.
    retry_attempts: AtomicUsize,
}

impl std::fmt::Debug for FallbackManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FallbackManager")
            .field("config", &self.config)
            .field("current_level", &self.current_level())
            .field("consecutive_failures", &self.consecutive_failures.load(Ordering::Relaxed))
            .finish()
    }
}

impl FallbackManager {
    /// Create a new fallback manager with configuration.
    pub fn new(config: FallbackConfig) -> Self {
        Self {
            config,
            current_level: AtomicU8::new(0),
            consecutive_failures: AtomicUsize::new(0),
            consecutive_successes: AtomicUsize::new(0),
            retry_attempts: AtomicUsize::new(0),
        }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(FallbackConfig::default())
    }

    /// Get current fallback level.
    pub fn current_level(&self) -> FallbackLevel {
        self.current_level.load(Ordering::Relaxed).into()
    }

    /// Check if we're at algorithm-only level.
    pub fn is_algorithm_only(&self) -> bool {
        self.current_level() == FallbackLevel::AlgorithmOnly
    }

    /// Check if we should use simplified context.
    pub fn should_simplify(&self) -> bool {
        matches!(
            self.current_level(),
            FallbackLevel::Simplified | FallbackLevel::AlgorithmOnly
        )
    }

    /// Get delay for next retry based on attempt number.
    pub fn retry_delay(&self, attempt: usize) -> Duration {
        let delay = self.config.initial_delay_ms as f64
            * self.config.backoff_multiplier.powi(attempt as i32);
        let delay = delay.min(self.config.max_delay_ms as f64);
        Duration::from_millis(delay as u64)
    }

    /// Record a successful operation.
    ///
    /// May de-escalate the fallback level after consecutive successes.
    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);

        let successes = self.consecutive_successes.fetch_add(1, Ordering::Relaxed) + 1;

        // De-escalate after enough consecutive successes
        if successes >= self.config.successes_before_deescalate {
            let current = self.current_level.load(Ordering::Relaxed);
            if current > 0 {
                self.current_level.fetch_sub(1, Ordering::Relaxed);
                debug!("Fallback level de-escalated to {:?}", self.current_level());
            }
            self.consecutive_successes.store(0, Ordering::Relaxed);
        }
    }

    /// Record a failure and potentially escalate level.
    ///
    /// Returns the recommended action.
    pub fn record_failure(&self, error: &FallbackError) -> FallbackAction {
        self.consecutive_successes.store(0, Ordering::Relaxed);

        // Check if we should escalate
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;

        if failures >= self.config.failures_before_escalate {
            self.escalate_level();
            self.consecutive_failures.store(0, Ordering::Relaxed);
        }

        // Determine action based on error and current level
        match error {
            FallbackError::Network(_) | FallbackError::RateLimited => {
                if self.retry_attempts.load(Ordering::Relaxed) < self.config.max_retries {
                    FallbackAction::Retry
                } else {
                    FallbackAction::Escalate
                }
            }
            FallbackError::TokenLimitExceeded => FallbackAction::Simplify,
            FallbackError::Unavailable(_) | FallbackError::Exhausted => {
                FallbackAction::UseAlgorithm
            }
            FallbackError::ParseError(_) => {
                // Try default decision, don't escalate
                FallbackAction::UseDefault
            }
        }
    }

    /// Escalate to next fallback level.
    fn escalate_level(&self) {
        let current = self.current_level.load(Ordering::Relaxed);
        if current < 3 {
            self.current_level.fetch_add(1, Ordering::Relaxed);
            warn!("Fallback level escalated to {:?}", self.current_level());
        }
    }

    /// Start a retry attempt.
    pub fn start_retry(&self) {
        self.retry_attempts.fetch_add(1, Ordering::Relaxed);
    }

    /// Reset retry counter (after successful operation).
    pub fn reset_retry_count(&self) {
        self.retry_attempts.store(0, Ordering::Relaxed);
    }

    /// Reset all state for new query.
    pub fn reset(&self) {
        self.current_level.store(0, Ordering::Relaxed);
        self.consecutive_failures.store(0, Ordering::Relaxed);
        self.consecutive_successes.store(0, Ordering::Relaxed);
        self.retry_attempts.store(0, Ordering::Relaxed);
    }

    /// Get current statistics.
    pub fn stats(&self) -> FallbackStats {
        FallbackStats {
            current_level: self.current_level(),
            ..Default::default()
        }
    }

    /// Get the configuration.
    pub fn config(&self) -> &FallbackConfig {
        &self.config
    }
}

/// Action to take after a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackAction {
    /// Retry the operation (with backoff).
    Retry,
    /// Simplify context and retry.
    Simplify,
    /// Escalate to next fallback level.
    Escalate,
    /// Use algorithm-only mode.
    UseAlgorithm,
    /// Use a default decision.
    UseDefault,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_level_conversion() {
        assert_eq!(FallbackLevel::from(0), FallbackLevel::Normal);
        assert_eq!(FallbackLevel::from(1), FallbackLevel::Retry);
        assert_eq!(FallbackLevel::from(2), FallbackLevel::Simplified);
        assert_eq!(FallbackLevel::from(3), FallbackLevel::AlgorithmOnly);
        assert_eq!(FallbackLevel::from(4), FallbackLevel::AlgorithmOnly);
    }

    #[test]
    fn test_fallback_manager_creation() {
        let manager = FallbackManager::with_defaults();
        assert_eq!(manager.current_level(), FallbackLevel::Normal);
        assert!(!manager.is_algorithm_only());
        assert!(!manager.should_simplify());
    }

    #[test]
    fn test_retry_delay() {
        let manager = FallbackManager::with_defaults();

        let d0 = manager.retry_delay(0);
        let d1 = manager.retry_delay(1);
        let d2 = manager.retry_delay(2);

        assert!(d1 > d0);
        assert!(d2 > d1);
    }

    #[test]
    fn test_retry_delay_max() {
        let config = FallbackConfig {
            max_delay_ms: 5000,
            ..Default::default()
        };
        let manager = FallbackManager::new(config);

        // High attempt should cap at max
        let delay = manager.retry_delay(10);
        assert!(delay.as_millis() <= 5000);
    }

    #[test]
    fn test_record_success() {
        let manager = FallbackManager::with_defaults();
        manager.current_level.store(1, Ordering::Relaxed);

        // Need multiple successes to de-escalate
        for _ in 0..manager.config.successes_before_deescalate {
            manager.record_success();
        }

        assert_eq!(manager.current_level(), FallbackLevel::Normal);
    }

    #[test]
    fn test_record_failure_escalate() {
        let manager = FallbackManager::with_defaults();

        // Trigger failures to escalate
        for _ in 0..manager.config.failures_before_escalate {
            let action = manager.record_failure(&FallbackError::Network("test".to_string()));
            assert!(matches!(action, FallbackAction::Retry | FallbackAction::Escalate));
        }

        assert_eq!(manager.current_level(), FallbackLevel::Retry);
    }

    #[test]
    fn test_record_failure_token_limit() {
        let manager = FallbackManager::with_defaults();

        let action = manager.record_failure(&FallbackError::TokenLimitExceeded);
        assert_eq!(action, FallbackAction::Simplify);
    }

    #[test]
    fn test_record_failure_unavailable() {
        let manager = FallbackManager::with_defaults();

        let action = manager.record_failure(&FallbackError::Unavailable("test".to_string()));
        assert_eq!(action, FallbackAction::UseAlgorithm);
    }

    #[test]
    fn test_reset() {
        let manager = FallbackManager::with_defaults();

        // Escalate level
        manager.current_level.store(3, Ordering::Relaxed);
        manager.consecutive_failures.store(5, Ordering::Relaxed);

        manager.reset();

        assert_eq!(manager.current_level(), FallbackLevel::Normal);
        assert_eq!(manager.consecutive_failures.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_error_retryable() {
        assert!(FallbackError::Network("test".to_string()).is_retryable());
        assert!(FallbackError::RateLimited.is_retryable());
        assert!(!FallbackError::TokenLimitExceeded.is_retryable());
        assert!(!FallbackError::Unavailable("test".to_string()).is_retryable());
    }

    #[test]
    fn test_error_needs_simplification() {
        assert!(FallbackError::TokenLimitExceeded.needs_simplification());
        assert!(!FallbackError::Network("test".to_string()).needs_simplification());
    }
}
