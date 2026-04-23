// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Concurrency control for LLM API calls.
//!
//! Combines semaphore (concurrency limit) with token-bucket rate limiter (RPM).

use std::num::NonZeroU32;
use std::sync::Arc;

use governor::{
    Quota, RateLimiter as GovernorLimiter,
    clock::{Clock, DefaultClock},
    state::{InMemoryState, NotKeyed},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{Semaphore, SemaphorePermit};
use tracing::{debug, trace};

// ============================================================
// ConcurrencyConfig
// ============================================================

/// Concurrency control configuration.
///
/// Controls how LLM requests are rate-limited and throttled
/// to avoid overwhelming the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Maximum concurrent LLM API calls.
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,

    /// Rate limit: requests per minute (token bucket).
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: usize,

    /// Whether rate limiting is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether semaphore-based concurrency limiting is enabled.
    #[serde(default = "default_true")]
    pub semaphore_enabled: bool,
}

impl From<&vectorless_config::ThrottleConfig> for ConcurrencyConfig {
    fn from(c: &vectorless_config::ThrottleConfig) -> Self {
        Self {
            max_concurrent_requests: c.max_concurrent_requests,
            requests_per_minute: c.requests_per_minute,
            enabled: c.enabled,
            semaphore_enabled: c.semaphore_enabled,
        }
    }
}

fn default_max_concurrent_requests() -> usize {
    10
}
fn default_requests_per_minute() -> usize {
    500
}
fn default_true() -> bool {
    true
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: default_max_concurrent_requests(),
            requests_per_minute: default_requests_per_minute(),
            enabled: true,
            semaphore_enabled: true,
        }
    }
}

impl ConcurrencyConfig {
    /// Create a new config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum concurrent requests.
    pub fn with_max_concurrent_requests(mut self, max: usize) -> Self {
        self.max_concurrent_requests = max;
        self
    }

    /// Set the requests per minute rate limit.
    pub fn with_requests_per_minute(mut self, rpm: usize) -> Self {
        self.requests_per_minute = rpm;
        self
    }

    /// Enable or disable rate limiting.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Create a config for conservative scenarios.
    pub fn conservative() -> Self {
        Self {
            max_concurrent_requests: 5,
            requests_per_minute: 100,
            enabled: true,
            semaphore_enabled: true,
        }
    }

    /// Create a config that disables all limits.
    pub fn unlimited() -> Self {
        Self {
            max_concurrent_requests: usize::MAX,
            requests_per_minute: usize::MAX,
            enabled: false,
            semaphore_enabled: false,
        }
    }
}

// ============================================================
// ConcurrencyController
// ============================================================

/// Concurrency controller for LLM API calls.
///
/// Combines:
/// - **Rate Limiter** — Token bucket to limit requests per time period
/// - **Semaphore** — Limit concurrent requests
///
/// The only operation needed by business code is [`acquire()`](ConcurrencyController::acquire).
#[derive(Clone)]
pub struct ConcurrencyController {
    semaphore: Arc<Semaphore>,
    rate_limiter: Option<Arc<GovernorLimiter<NotKeyed, InMemoryState, DefaultClock>>>,
    semaphore_enabled: bool,
}

impl ConcurrencyController {
    /// Create a new concurrency controller with the given configuration.
    pub fn new(config: ConcurrencyConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_requests));
        let rate_limiter = if config.enabled {
            let rpm = NonZeroU32::new(config.requests_per_minute as u32)
                .unwrap_or_else(|| NonZeroU32::new(1).unwrap());
            Some(Arc::new(GovernorLimiter::direct(Quota::per_minute(rpm))))
        } else {
            None
        };

        Self {
            semaphore,
            rate_limiter,
            semaphore_enabled: config.semaphore_enabled,
        }
    }

    /// Create a controller with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ConcurrencyConfig::default())
    }

    /// Acquire a permit for making an LLM request.
    ///
    /// This will:
    /// 1. Wait for the rate limiter (if enabled)
    /// 2. Acquire a semaphore permit (if enabled)
    ///
    /// The permit is automatically released when dropped.
    pub async fn acquire(&self) -> Option<SemaphorePermit<'_>> {
        // Step 1: Wait for rate limiter
        if let Some(ref limiter) = self.rate_limiter {
            let clock = DefaultClock::default();
            loop {
                match limiter.check() {
                    Ok(_) => {
                        trace!("Rate limiter: token acquired");
                        break;
                    }
                    Err(negative) => {
                        let wait_duration = negative.wait_time_from(clock.now());
                        trace!(
                            wait_ms = wait_duration.as_millis() as u64,
                            "Rate limiter: waiting for token"
                        );
                        tokio::time::sleep(wait_duration).await;
                    }
                }
            }
            debug!("Rate limiter: token acquired");
        }

        // Step 2: Acquire semaphore permit
        if self.semaphore_enabled {
            trace!("Waiting for semaphore permit");
            let permit = self
                .semaphore
                .acquire()
                .await
                .expect("semaphore should not be closed");
            debug!(
                "Semaphore: permit acquired (available: {})",
                self.semaphore.available_permits()
            );
            Some(permit)
        } else {
            None
        }
    }
}

impl std::fmt::Debug for ConcurrencyController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConcurrencyController")
            .field("available_permits", &self.semaphore.available_permits())
            .field("has_rate_limiter", &self.rate_limiter.is_some())
            .field("semaphore_enabled", &self.semaphore_enabled)
            .finish()
    }
}

impl Default for ConcurrencyController {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_controller_acquire() {
        let controller = ConcurrencyController::new(ConcurrencyConfig {
            max_concurrent_requests: 2,
            requests_per_minute: 100,
            enabled: false,
            semaphore_enabled: true,
        });

        let permit1 = controller.acquire().await;
        assert!(permit1.is_some());

        let permit2 = controller.acquire().await;
        assert!(permit2.is_some());

        drop(permit1);
    }

    #[test]
    fn test_controller_creation() {
        let controller = ConcurrencyController::with_defaults();
        assert!(controller.semaphore.available_permits() > 0);
    }

    #[test]
    fn test_rate_limiter_creation() {
        let config = ConcurrencyConfig {
            max_concurrent_requests: 10,
            requests_per_minute: 100,
            enabled: true,
            semaphore_enabled: true,
        };
        let controller = ConcurrencyController::new(config);
        assert!(controller.rate_limiter.is_some());
    }
}
