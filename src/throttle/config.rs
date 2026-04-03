// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Concurrency control configuration types.

use serde::{Deserialize, Serialize};

/// Concurrency control configuration.
///
/// This controls how LLM requests are rate-limited and throttled
/// to avoid overwhelming the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Maximum concurrent LLM API calls.
    ///
    /// This limits how many requests can be in-flight at the same time.
    /// Default: 10
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,

    /// Rate limit: requests per minute.
    ///
    /// This is a soft limit using token bucket algorithm.
    /// Default: 500 (OpenAI default tier)
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: usize,

    /// Alias for `enabled` - whether rate limiting is enabled.
    ///
    /// When disabled, only semaphore-based concurrency control is used.
    /// Default: true
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether to enable concurrency limiting via semaphore.
    ///
    /// When disabled, only rate limiting is used.
    /// Default: true
    #[serde(default = "default_true")]
    pub semaphore_enabled: bool,
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

    /// Create a config for high-throughput scenarios.
    ///
    /// Uses higher limits suitable for paid API tiers.
    pub fn high_throughput() -> Self {
        Self {
            max_concurrent_requests: 50,
            requests_per_minute: 3000,
            enabled: true,
            semaphore_enabled: true,
        }
    }

    /// Create a config for conservative scenarios.
    ///
    /// Uses lower limits to avoid rate limit errors.
    pub fn conservative() -> Self {
        Self {
            max_concurrent_requests: 5,
            requests_per_minute: 100,
            enabled: true,
            semaphore_enabled: true,
        }
    }

    /// Create a config that disables all limits.
    ///
    /// Useful for testing or when external rate limiting is used.
    pub fn unlimited() -> Self {
        Self {
            max_concurrent_requests: usize::MAX,
            requests_per_minute: usize::MAX,
            enabled: false,
            semaphore_enabled: false,
        }
    }
}
