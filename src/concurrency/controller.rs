// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Concurrency controller combining semaphore and rate limiter.

use std::sync::Arc;
use tokio::sync::{Semaphore, SemaphorePermit};
use tracing::{debug, trace};

use super::config::ConcurrencyConfig;
use super::rate_limiter::RateLimiter;

/// Concurrency controller for LLM API calls.
///
/// Combines:
/// - **Rate Limiter** — Token bucket to limit requests per time period
/// - **Semaphore** — Limit concurrent requests
///
/// # Example
///
/// ```rust
/// use vectorless::concurrency::{ConcurrencyController, ConcurrencyConfig};
///
/// # #[tokio::main]
/// # async fn main() {
/// let config = ConcurrencyConfig::default();
/// let controller = ConcurrencyController::new(config);
///
/// // Before making an API call
/// let permit = controller.acquire().await;
///
/// // Make the API call...
/// drop(permit); // Release when done
/// # }
/// ```
#[derive(Clone)]
pub struct ConcurrencyController {
    /// Semaphore for limiting concurrent requests.
    semaphore: Arc<Semaphore>,
    /// Rate limiter for throttling requests.
    rate_limiter: Option<Arc<RateLimiter>>,
    /// Configuration.
    config: ConcurrencyConfig,
}

impl ConcurrencyController {
    /// Create a new concurrency controller with the given configuration.
    pub fn new(config: ConcurrencyConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_requests));
        let rate_limiter = if config.enabled {
            Some(Arc::new(RateLimiter::new(config.requests_per_minute)))
        } else {
            None
        };

        Self {
            semaphore,
            rate_limiter,
            config,
        }
    }

    /// Create a controller with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ConcurrencyConfig::default())
    }

    /// Create a controller for high-throughput scenarios.
    pub fn high_throughput() -> Self {
        Self::new(ConcurrencyConfig::high_throughput())
    }

    /// Create a controller for conservative scenarios.
    pub fn conservative() -> Self {
        Self::new(ConcurrencyConfig::conservative())
    }

    /// Create a controller with no limits.
    pub fn unlimited() -> Self {
        Self::new(ConcurrencyConfig::unlimited())
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
            trace!("Waiting for rate limiter");
            limiter.acquire().await;
            debug!("Rate limiter: token acquired");
        }

        // Step 2: Acquire semaphore permit
        if self.config.semaphore_enabled {
            trace!("Waiting for semaphore permit");
            let permit = self.semaphore.acquire().await.unwrap();
            debug!("Semaphore: permit acquired (available: {})", self.semaphore.available_permits());
            Some(permit)
        } else {
            None
        }
    }

    /// Try to acquire a permit without waiting.
    ///
    /// Returns `None` if the limit is reached.
    pub fn try_acquire(&self) -> Option<SemaphorePermit<'_>> {
        // Check rate limiter
        if let Some(ref limiter) = self.rate_limiter {
            if !limiter.try_acquire() {
                return None;
            }
        }

        // Try to acquire semaphore
        if self.config.semaphore_enabled {
            self.semaphore.try_acquire().ok()
        } else {
            None
        }
    }

    /// Get the number of available semaphore permits.
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Get the configuration.
    pub fn config(&self) -> &ConcurrencyConfig {
        &self.config
    }

    /// Get the rate limiter (if any).
    pub fn rate_limiter(&self) -> Option<&RateLimiter> {
        self.rate_limiter.as_deref()
    }
}

impl std::fmt::Debug for ConcurrencyController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConcurrencyController")
            .field("max_concurrent_requests", &self.config.max_concurrent_requests)
            .field("requests_per_minute", &self.config.requests_per_minute)
            .field("rate_limiting_enabled", &self.config.enabled)
            .field("semaphore_enabled", &self.config.semaphore_enabled)
            .field("available_permits", &self.semaphore.available_permits())
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
            enabled: false, // Disable rate limiting for faster test
            semaphore_enabled: true,
        });

        let permit1 = controller.acquire().await;
        assert!(permit1.is_some());
        assert_eq!(controller.available_permits(), 1);

        let permit2 = controller.acquire().await;
        assert!(permit2.is_some());
        assert_eq!(controller.available_permits(), 0);

        drop(permit1);
        assert_eq!(controller.available_permits(), 1);
    }

    #[test]
    fn test_controller_creation() {
        let controller = ConcurrencyController::with_defaults();
        assert!(controller.available_permits() > 0);
    }
}
