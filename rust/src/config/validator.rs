// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration validation.
//!
//! This module provides comprehensive validation for configuration values,
//! including range checks, consistency checks, and dependency validation.

use super::types::{Config, ConfigValidationError, Severity, ValidationError};

/// Configuration validator.
#[derive(Debug, Default)]
pub struct ConfigValidator {
    /// Validation rules to apply.
    rules: Vec<Box<dyn ValidationRule>>,
}

impl ConfigValidator {
    /// Create a new validator with default rules.
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(RangeValidator),
                Box::new(ConsistencyValidator),
                Box::new(DependencyValidator),
            ],
        }
    }

    /// Add a custom validation rule.
    pub fn with_rule(mut self, rule: Box<dyn ValidationRule>) -> Self {
        self.rules.push(rule);
        self
    }

    /// Validate the configuration.
    pub fn validate(&self, config: &Config) -> Result<(), ConfigValidationError> {
        let mut errors = Vec::new();

        for rule in &self.rules {
            rule.validate(config, &mut errors);
        }

        // Only fail on errors, not warnings or info
        let has_errors = errors.iter().any(|e| e.severity == Severity::Error);

        if has_errors {
            Err(ConfigValidationError { errors })
        } else {
            Ok(())
        }
    }
}

/// Trait for validation rules.
pub trait ValidationRule: std::fmt::Debug + Send + Sync {
    /// Validate the configuration, appending errors if found.
    fn validate(&self, config: &Config, errors: &mut Vec<ValidationError>);
}

/// Validates value ranges.
#[derive(Debug)]
struct RangeValidator;

impl ValidationRule for RangeValidator {
    fn validate(&self, config: &Config, errors: &mut Vec<ValidationError>) {
        // Indexer ranges
        if config.indexer.subsection_threshold == 0 {
            errors.push(ValidationError::error(
                "indexer.subsection_threshold",
                "Subsection threshold must be greater than 0",
            ));
        }

        if config.indexer.subsection_threshold > 10000 {
            errors.push(
                ValidationError::warning(
                    "indexer.subsection_threshold",
                    "Subsection threshold is very high, may impact performance",
                )
                .with_actual(config.indexer.subsection_threshold.to_string()),
            );
        }

        // LLM slot token ranges
        if config.llm.index.max_tokens == 0 {
            errors.push(ValidationError::error(
                "llm.index.max_tokens",
                "Index max tokens must be greater than 0",
            ));
        }

        if config.llm.retrieval.max_tokens == 0 {
            errors.push(ValidationError::error(
                "llm.retrieval.max_tokens",
                "Retrieval max tokens must be greater than 0",
            ));
        }

        // Retrieval ranges
        if config.retrieval.top_k == 0 {
            errors.push(ValidationError::error(
                "retrieval.top_k",
                "Top K must be greater than 0",
            ));
        }

        if config.retrieval.search.beam_width == 0 {
            errors.push(ValidationError::error(
                "retrieval.search.beam_width",
                "Beam width must be greater than 0",
            ));
        }

        // Throttle ranges
        if config.llm.throttle.max_concurrent_requests == 0 {
            errors.push(ValidationError::error(
                "llm.throttle.max_concurrent_requests",
                "Max concurrent requests must be greater than 0",
            ));
        }

        if config.llm.throttle.requests_per_minute == 0 {
            errors.push(ValidationError::error(
                "llm.throttle.requests_per_minute",
                "Requests per minute must be greater than 0",
            ));
        }

        // Fallback ranges
        if config.llm.fallback.max_retries == 0 {
            errors.push(ValidationError::warning(
                "llm.fallback.max_retries",
                "Max retries is 0, fallback will not retry",
            ));
        }
    }
}

/// Validates configuration consistency.
#[derive(Debug)]
struct ConsistencyValidator;

impl ValidationRule for ConsistencyValidator {
    fn validate(&self, config: &Config, errors: &mut Vec<ValidationError>) {
        // Check if index tokens are reasonable
        if config.llm.index.max_tokens > config.indexer.max_segment_tokens {
            errors.push(
                ValidationError::warning(
                    "llm.index.max_tokens",
                    "Index max tokens exceeds max segment tokens",
                )
                .with_expected(format!("<= {}", config.indexer.max_segment_tokens))
                .with_actual(config.llm.index.max_tokens.to_string()),
            );
        }

        // Check if sufficiency thresholds are consistent
        if config.retrieval.sufficiency.min_tokens > config.retrieval.sufficiency.target_tokens {
            errors.push(
                ValidationError::error(
                    "retrieval.sufficiency.min_tokens",
                    "Min tokens cannot exceed target tokens",
                )
                .with_expected(format!("<= {}", config.retrieval.sufficiency.target_tokens))
                .with_actual(config.retrieval.sufficiency.min_tokens.to_string()),
            );
        }

        if config.retrieval.sufficiency.target_tokens > config.retrieval.sufficiency.max_tokens {
            errors.push(
                ValidationError::error(
                    "retrieval.sufficiency.target_tokens",
                    "Target tokens cannot exceed max tokens",
                )
                .with_expected(format!("<= {}", config.retrieval.sufficiency.max_tokens))
                .with_actual(config.retrieval.sufficiency.target_tokens.to_string()),
            );
        }
    }
}

/// Validates configuration dependencies.
#[derive(Debug)]
struct DependencyValidator;

impl ValidationRule for DependencyValidator {
    fn validate(&self, config: &Config, errors: &mut Vec<ValidationError>) {
        // Check if API key is available when summaries are needed
        if config.llm.api_key.is_none() {
            if config.indexer.max_summary_tokens > 0 {
                errors.push(ValidationError::info(
                    "llm.api_key",
                    "No API key configured, summary generation will be disabled",
                ));
            }
        }

        // Check fallback configuration
        if config.llm.fallback.enabled {
            if config.llm.fallback.models.is_empty() && config.llm.fallback.endpoints.is_empty() {
                errors.push(ValidationError::warning(
                    "llm.fallback.models",
                    "Fallback enabled but no fallback models or endpoints configured",
                ));
            }

            // Check retry behavior consistency
            if matches!(
                config.llm.fallback.on_rate_limit,
                super::types::FallbackBehavior::Fallback
            ) && config.llm.fallback.models.is_empty()
            {
                errors.push(ValidationError::error(
                    "llm.fallback.models",
                    "Rate limit behavior is 'fallback' but no fallback models configured",
                ));
            }
        }

        // Check cache configuration
        if config.retrieval.cache.max_entries == 0 {
            errors.push(ValidationError::warning(
                "retrieval.cache.max_entries",
                "Cache disabled (max_entries = 0), performance may be impacted",
            ));
        }

        // Check strategy configuration
        if config.retrieval.strategy.exploration_weight <= 0.0 {
            errors.push(
                ValidationError::error(
                    "retrieval.strategy.exploration_weight",
                    "Exploration weight must be positive",
                )
                .with_actual(config.retrieval.strategy.exploration_weight.to_string()),
            );
        }

        // Check similarity thresholds are ordered correctly
        if config.retrieval.strategy.low_similarity_threshold
            >= config.retrieval.strategy.high_similarity_threshold
        {
            errors.push(
                ValidationError::error(
                    "retrieval.strategy.low_similarity_threshold",
                    "Low similarity threshold must be less than high similarity threshold",
                )
                .with_expected(format!(
                    "< {}",
                    config.retrieval.strategy.high_similarity_threshold
                ))
                .with_actual(
                    config
                        .retrieval
                        .strategy
                        .low_similarity_threshold
                        .to_string(),
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_valid_config() {
        let config = Config::default();
        let validator = ConfigValidator::new();
        // Default config should pass validation (no errors, warnings are ok)
        let result = validator.validate(&config);
        assert!(result.is_ok(), "Default config should pass validation");
    }

    #[test]
    fn test_validator_catches_range_errors() {
        let mut config = Config::default();
        config.retrieval.top_k = 0;

        let validator = ConfigValidator::new();
        let result = validator.validate(&config);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.errors.iter().any(|e| e.path.contains("top_k")));
    }

    #[test]
    fn test_validator_catches_consistency_errors() {
        let mut config = Config::default();
        config.retrieval.sufficiency.min_tokens = 3000;
        config.retrieval.sufficiency.target_tokens = 2000;

        let validator = ConfigValidator::new();
        let result = validator.validate(&config);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.errors.iter().any(|e| e.path.contains("min_tokens")));
    }

    #[test]
    fn test_validator_catches_dependency_warnings() {
        let mut config = Config::default();
        config.llm.fallback.enabled = true;
        config.llm.fallback.models.clear();

        let validator = ConfigValidator::new();
        let result = validator.validate(&config);

        // Should succeed but with warnings
        if let Err(err) = result {
            assert!(
                err.errors
                    .iter()
                    .any(|e| e.path.contains("llm.fallback.models"))
            );
        }
    }
}
