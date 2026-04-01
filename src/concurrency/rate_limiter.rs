// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Rate limiter using token bucket algorithm (governor).

use governor::{
    clock::{Clock, DefaultClock},
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter as GovernorLimiter,
};
use std::num::NonZeroU32;
use std::sync::Arc;
use tracing::trace;

/// Rate limiter for API calls.
///
/// Uses the governor library's token bucket algorithm to limit
/// the rate of API requests.
#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<GovernorLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    requests_per_minute: usize,
}

impl RateLimiter {
    /// Create a new rate limiter with the given requests per minute.
    ///
    /// # Arguments
    ///
    /// * `requests_per_minute` - Maximum number of requests allowed per minute.
    ///
    /// # Example
    ///
    /// ```
    /// use vectorless::concurrency::RateLimiter;
    ///
    /// let limiter = RateLimiter::new(100); // 100 requests per minute
    /// ```
    pub fn new(requests_per_minute: usize) -> Self {
        let rpm = NonZeroU32::new(requests_per_minute as u32)
            .unwrap_or_else(|| NonZeroU32::new(1).unwrap());

        let quota = Quota::per_minute(rpm);
        let inner = Arc::new(GovernorLimiter::direct(quota));

        Self {
            inner,
            requests_per_minute,
        }
    }

    /// Wait until a request can be made.
    ///
    /// This method will block until the rate limiter has an available token.
    pub async fn acquire(&self) {
        let clock = DefaultClock::default();
        loop {
            match self.inner.check() {
                Ok(_) => {
                    trace!("Rate limiter: token acquired");
                    return;
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
    }

    /// Try to acquire a token without waiting.
    ///
    /// Returns `true` if a token was acquired, `false` if the limit is reached.
    pub fn try_acquire(&self) -> bool {
        self.inner.check().is_ok()
    }

    /// Get the configured requests per minute.
    pub fn requests_per_minute(&self) -> usize {
        self.requests_per_minute
    }
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter")
            .field("requests_per_minute", &self.requests_per_minute)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_creation() {
        let limiter = RateLimiter::new(100);
        assert_eq!(limiter.requests_per_minute(), 100);
    }

    #[test]
    fn test_try_acquire() {
        let limiter = RateLimiter::new(10);
        // Should be able to acquire at least one token
        assert!(limiter.try_acquire());
    }
}
