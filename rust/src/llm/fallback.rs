// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Fallback and error recovery for LLM calls.
//!
//! This module provides graceful degradation when LLM API calls fail:
//! - Automatic model switching (e.g., gpt-4o → gpt-4o-mini)
//! - Automatic endpoint switching
//! - Configurable retry and fallback behaviors
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::llm::fallback::{FallbackChain, FallbackConfig};
//!
//! let config = FallbackConfig::default();
//! let chain = FallbackChain::new(config);
//!
//! // Check if fallback is enabled
//! assert!(chain.is_enabled());
//! ```

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::error::LlmError;
use crate::config::{
    FallbackBehavior, FallbackConfig as ConfigFallbackConfig, OnAllFailedBehavior,
};

/// Result from a fallback-aware LLM call.
#[derive(Debug, Clone)]
pub struct FallbackResult<T> {
    /// The actual result.
    pub result: T,
    /// Whether the result came from a fallback model/endpoint.
    pub degraded: bool,
    /// The model that was ultimately used.
    pub model: String,
    /// The endpoint that was ultimately used.
    pub endpoint: String,
    /// History of fallback attempts (for debugging).
    pub fallback_history: Vec<FallbackStep>,
}

impl<T> FallbackResult<T> {
    /// Create a successful result without fallback.
    pub fn success(result: T, model: String, endpoint: String) -> Self {
        Self {
            result,
            degraded: false,
            model,
            endpoint,
            fallback_history: Vec::new(),
        }
    }

    /// Create a result from a fallback.
    pub fn from_fallback(
        result: T,
        model: String,
        endpoint: String,
        history: Vec<FallbackStep>,
    ) -> Self {
        Self {
            result,
            degraded: true,
            model,
            endpoint,
            fallback_history: history,
        }
    }
}

/// A single step in the fallback chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackStep {
    /// The model we tried.
    pub from_model: String,
    /// The model we fell back to (if any).
    pub to_model: Option<String>,
    /// The endpoint we tried.
    pub from_endpoint: String,
    /// The endpoint we fell back to (if any).
    pub to_endpoint: Option<String>,
    /// The reason for fallback.
    pub reason: String,
}

/// Fallback chain manager.
#[derive(Debug, Clone)]
pub struct FallbackChain {
    config: FallbackConfig,
}

/// Runtime fallback configuration (converted from config::FallbackConfig).
#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// Whether fallback is enabled.
    pub enabled: bool,
    /// Fallback models in priority order.
    pub models: Vec<String>,
    /// Fallback endpoints in priority order.
    pub endpoints: Vec<String>,
    /// Behavior on rate limit error.
    pub on_rate_limit: FallbackBehavior,
    /// Behavior on timeout error.
    pub on_timeout: FallbackBehavior,
    /// Behavior when all attempts fail.
    pub on_all_failed: OnAllFailedBehavior,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            models: vec!["gpt-4o-mini".to_string(), "glm-4-flash".to_string()],
            endpoints: vec![],
            on_rate_limit: FallbackBehavior::RetryThenFallback,
            on_timeout: FallbackBehavior::RetryThenFallback,
            on_all_failed: OnAllFailedBehavior::ReturnError,
        }
    }
}

impl From<ConfigFallbackConfig> for FallbackConfig {
    fn from(config: ConfigFallbackConfig) -> Self {
        Self {
            enabled: config.enabled,
            models: config.models,
            endpoints: config.endpoints,
            on_rate_limit: config.on_rate_limit,
            on_timeout: config.on_timeout,
            on_all_failed: config.on_all_failed,
        }
    }
}

impl FallbackConfig {
    /// Create a new fallback config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable fallback.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }
}

impl FallbackChain {
    /// Create a new fallback chain with the given configuration.
    pub fn new(config: FallbackConfig) -> Self {
        Self { config }
    }

    /// Create a disabled fallback chain (no fallback).
    pub fn disabled() -> Self {
        Self::new(FallbackConfig::disabled())
    }

    /// Get the configuration.
    pub fn config(&self) -> &FallbackConfig {
        &self.config
    }

    /// Check if fallback is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Determine the appropriate behavior for an error.
    pub fn behavior_for_error(&self, error: &LlmError) -> FallbackBehavior {
        match error {
            LlmError::RateLimit(_) => self.config.on_rate_limit,
            LlmError::Timeout(_) => self.config.on_timeout,
            _ => FallbackBehavior::Fail,
        }
    }

    /// Check if an error should trigger fallback.
    pub fn should_fallback(&self, error: &LlmError) -> bool {
        if !self.config.enabled {
            return false;
        }

        match self.behavior_for_error(error) {
            FallbackBehavior::Fallback | FallbackBehavior::RetryThenFallback => true,
            FallbackBehavior::Retry | FallbackBehavior::Fail => false,
        }
    }

    /// Check if an error should trigger retry.
    pub fn should_retry(&self, error: &LlmError) -> bool {
        if !self.config.enabled {
            return false;
        }

        match self.behavior_for_error(error) {
            FallbackBehavior::Retry | FallbackBehavior::RetryThenFallback => true,
            FallbackBehavior::Fallback | FallbackBehavior::Fail => false,
        }
    }

    /// Get the next fallback model.
    pub fn next_model(&self, current: &str) -> Option<String> {
        let models = &self.config.models;
        let current_idx = models.iter().position(|m| m == current);

        match current_idx {
            // Current model is in the list, try next one
            Some(idx) if idx + 1 < models.len() => {
                let next = models[idx + 1].clone();
                info!(from = current, to = %next, "Falling back to next model");
                Some(next)
            }
            // Current model is the last in the list, no more fallbacks
            Some(_) => {
                warn!(
                    model = current,
                    "Already at last fallback model, no more available"
                );
                None
            }
            // Current model not in fallback list, try first fallback
            None => {
                if !models.is_empty() && models[0] != current {
                    let next = models[0].clone();
                    info!(from = current, to = %next, "Falling back to first fallback model");
                    Some(next)
                } else {
                    warn!(model = current, "No more fallback models available");
                    None
                }
            }
        }
    }

    /// Get the next fallback endpoint.
    pub fn next_endpoint(&self, current: &str) -> Option<String> {
        let endpoints = &self.config.endpoints;
        let current_idx = endpoints.iter().position(|e| e == current);

        match current_idx {
            // Current endpoint is in the list, try next one
            Some(idx) if idx + 1 < endpoints.len() => {
                let next = endpoints[idx + 1].clone();
                info!(from = current, to = %next, "Falling back to next endpoint");
                Some(next)
            }
            // Current endpoint is the last in the list, no more fallbacks
            Some(_) => {
                warn!(
                    endpoint = current,
                    "Already at last fallback endpoint, no more available"
                );
                None
            }
            // Current endpoint not in fallback list, try first fallback
            None => {
                if !endpoints.is_empty() && endpoints[0] != current {
                    let next = endpoints[0].clone();
                    info!(from = current, to = %next, "Falling back to first fallback endpoint");
                    Some(next)
                } else {
                    debug!(endpoint = current, "No more fallback endpoints available");
                    None
                }
            }
        }
    }

    /// Record a fallback step.
    pub fn record_fallback(
        &self,
        history: &mut Vec<FallbackStep>,
        from_model: String,
        to_model: Option<String>,
        from_endpoint: String,
        to_endpoint: Option<String>,
        reason: String,
    ) {
        let step = FallbackStep {
            from_model,
            to_model,
            from_endpoint,
            to_endpoint,
            reason,
        };
        debug!(?step, "Recording fallback step");
        history.push(step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_config_default() {
        let config = FallbackConfig::default();
        assert!(config.enabled);
        assert!(!config.models.is_empty());
    }

    #[test]
    fn test_fallback_chain_disabled() {
        let chain = FallbackChain::disabled();
        assert!(!chain.is_enabled());
    }

    #[test]
    fn test_next_model() {
        let config = FallbackConfig {
            models: vec![
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
                "glm-4-flash".to_string(),
            ],
            ..FallbackConfig::default()
        };
        let chain = FallbackChain::new(config);

        // Should get next model in chain
        assert_eq!(chain.next_model("gpt-4o"), Some("gpt-4o-mini".to_string()));
        assert_eq!(
            chain.next_model("gpt-4o-mini"),
            Some("glm-4-flash".to_string())
        );
        assert_eq!(chain.next_model("glm-4-flash"), None);
    }

    #[test]
    fn test_next_model_not_in_list() {
        let config = FallbackConfig {
            models: vec!["gpt-4o-mini".to_string()],
            ..FallbackConfig::default()
        };
        let chain = FallbackChain::new(config);

        // Should fall back to first model in list
        assert_eq!(
            chain.next_model("unknown-model"),
            Some("gpt-4o-mini".to_string())
        );
    }

    #[test]
    fn test_behavior_for_rate_limit() {
        let config = FallbackConfig {
            on_rate_limit: FallbackBehavior::Fallback,
            ..FallbackConfig::default()
        };
        let chain = FallbackChain::new(config);

        let error = LlmError::RateLimit("Rate limited".to_string());
        assert_eq!(chain.behavior_for_error(&error), FallbackBehavior::Fallback);
    }

    #[test]
    fn test_should_fallback() {
        let config = FallbackConfig {
            enabled: true,
            on_rate_limit: FallbackBehavior::RetryThenFallback,
            ..FallbackConfig::default()
        };
        let chain = FallbackChain::new(config);

        let error = LlmError::RateLimit("Rate limited".to_string());
        assert!(chain.should_fallback(&error));

        let chain_disabled = FallbackChain::disabled();
        assert!(!chain_disabled.should_fallback(&error));
    }
}
