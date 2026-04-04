// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Concurrency control configuration types.

use serde::{Deserialize, Serialize};

/// Concurrency control configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Maximum concurrent LLM API calls.
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,

    /// Rate limit: requests per minute.
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: usize,

    /// Whether rate limiting is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether semaphore-based concurrency limiting is enabled.
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
            enabled: default_true(),
            semaphore_enabled: default_true(),
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

    /// Enable or disable semaphore.
    pub fn with_semaphore_enabled(mut self, enabled: bool) -> Self {
        self.semaphore_enabled = enabled;
        self
    }

    /// Convert to the runtime concurrency config.
    pub fn to_runtime_config(&self) -> crate::throttle::ConcurrencyConfig {
        crate::throttle::ConcurrencyConfig {
            max_concurrent_requests: self.max_concurrent_requests,
            requests_per_minute: self.requests_per_minute,
            enabled: self.enabled,
            semaphore_enabled: self.semaphore_enabled,
        }
    }
}

impl From<ConcurrencyConfig> for crate::throttle::ConcurrencyConfig {
    fn from(config: ConcurrencyConfig) -> Self {
        config.to_runtime_config()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrency_config_defaults() {
        let config = ConcurrencyConfig::default();
        assert_eq!(config.max_concurrent_requests, 10);
        assert_eq!(config.requests_per_minute, 500);
        assert!(config.enabled);
        assert!(config.semaphore_enabled);
    }

    #[test]
    fn test_concurrency_config_builder() {
        let config = ConcurrencyConfig::new()
            .with_max_concurrent_requests(20)
            .with_requests_per_minute(1000)
            .with_enabled(false);

        assert_eq!(config.max_concurrent_requests, 20);
        assert_eq!(config.requests_per_minute, 1000);
        assert!(!config.enabled);
    }
}
