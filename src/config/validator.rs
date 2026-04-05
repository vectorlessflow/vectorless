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

        // Summary ranges
        if config.summary.max_tokens == 0 {
            errors.push(ValidationError::error(
                "summary.max_tokens",
                "Summary max tokens must be greater than 0",
            ));
        }

        if config.summary.temperature < 0.0 || config.summary.temperature > 2.0 {
            errors.push(
                ValidationError::warning(
                    "summary.temperature",
                    "Temperature outside typical range [0.0, 2.0]",
                )
                .with_actual(config.summary.temperature.to_string()),
            );
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

        // Content aggregator ranges
        if config.retrieval.content.token_budget == 0 {
            errors.push(ValidationError::error(
                "retrieval.content.token_budget",
                "Token budget must be greater than 0",
            ));
        }

        if config.retrieval.content.min_relevance_score < 0.0
            || config.retrieval.content.min_relevance_score > 1.0
        {
            errors.push(
                ValidationError::error(
                    "retrieval.content.min_relevance_score",
                    "Min relevance score must be between 0.0 and 1.0",
                )
                .with_expected("0.0 - 1.0")
                .with_actual(config.retrieval.content.min_relevance_score.to_string()),
            );
        }

        if config.retrieval.content.hierarchical_min_per_level < 0.0
            || config.retrieval.content.hierarchical_min_per_level > 1.0
        {
            errors.push(ValidationError::error(
                "retrieval.content.hierarchical_min_per_level",
                "Hierarchical min per level must be between 0.0 and 1.0",
            ));
        }

        // Concurrency ranges
        if config.concurrency.max_concurrent_requests == 0 {
            errors.push(ValidationError::error(
                "concurrency.max_concurrent_requests",
                "Max concurrent requests must be greater than 0",
            ));
        }

        if config.concurrency.requests_per_minute == 0 {
            errors.push(ValidationError::error(
                "concurrency.requests_per_minute",
                "Requests per minute must be greater than 0",
            ));
        }

        // Fallback ranges
        if config.fallback.max_retries == 0 {
            errors.push(ValidationError::warning(
                "fallback.max_retries",
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
        // Check if summary tokens are reasonable
        if config.summary.max_tokens > config.indexer.max_segment_tokens {
            errors.push(
                ValidationError::warning(
                    "summary.max_tokens",
                    "Summary max tokens exceeds max segment tokens",
                )
                .with_expected(format!("<= {}", config.indexer.max_segment_tokens))
                .with_actual(config.summary.max_tokens.to_string()),
            );
        }

        // Check if content token budget is reasonable
        if config.retrieval.content.token_budget > 100000 {
            errors.push(
                ValidationError::warning(
                    "retrieval.content.token_budget",
                    "Token budget is very high, may cause performance issues",
                )
                .with_actual(config.retrieval.content.token_budget.to_string()),
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

        // Check scoring strategy validity
        let valid_strategies = ["keyword_only", "keyword_bm25", "hybrid"];
        if !valid_strategies.contains(&config.retrieval.content.scoring_strategy.as_str()) {
            errors.push(
                ValidationError::error(
                    "retrieval.content.scoring_strategy",
                    "Invalid scoring strategy",
                )
                .with_expected(format!("one of: {:?}", valid_strategies))
                .with_actual(config.retrieval.content.scoring_strategy.clone()),
            );
        }

        // Check output format validity
        let valid_formats = ["markdown", "json", "tree", "flat"];
        if !valid_formats.contains(&config.retrieval.content.output_format.as_str()) {
            errors.push(
                ValidationError::error("retrieval.content.output_format", "Invalid output format")
                    .with_expected(format!("one of: {:?}", valid_formats))
                    .with_actual(config.retrieval.content.output_format.clone()),
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
        if config.summary.api_key.is_none() {
            // Check if any feature requires LLM
            if config.indexer.max_summary_tokens > 0 {
                errors.push(ValidationError::info(
                    "summary.api_key",
                    "No API key configured, summary generation will be disabled",
                ));
            }
        }

        // Check fallback configuration
        if config.fallback.enabled {
            if config.fallback.models.is_empty() && config.fallback.endpoints.is_empty() {
                errors.push(ValidationError::warning(
                    "fallback.models",
                    "Fallback enabled but no fallback models or endpoints configured",
                ));
            }

            // Check retry behavior consistency
            if matches!(
                config.fallback.on_rate_limit,
                super::types::FallbackBehavior::Fallback
            ) && config.fallback.models.is_empty()
            {
                errors.push(ValidationError::error(
                    "fallback.models",
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
        config.retrieval.content.token_budget = 0;
        config.retrieval.content.min_relevance_score = 1.5;

        let validator = ConfigValidator::new();
        let result = validator.validate(&config);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.errors.iter().any(|e| e.path.contains("token_budget")));
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
        config.fallback.enabled = true;
        config.fallback.models.clear();

        let validator = ConfigValidator::new();
        let result = validator.validate(&config);

        // Should succeed but with warnings
        if let Err(err) = result {
            assert!(
                err.errors
                    .iter()
                    .any(|e| e.path.contains("fallback.models"))
            );
        }
    }
}
