// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Internal configuration management.
//!
//! Users configure vectorless via [`EngineBuilder`](crate::client::EngineBuilder) methods,
//! not by directly interacting with this module.

mod loader;
mod merge;
mod types;
mod validator;

pub(crate) use loader::{ConfigError, ConfigLoader};
pub(crate) use types::{
    CacheConfig, CompressionAlgorithm, CompressionConfig, ConcurrencyConfig, Config,
    ConfigValidationError, ContentAggregatorConfig, FallbackBehavior, FallbackConfig,
    IndexerConfig, LlmClientConfig, LlmConfig, LlmFallbackBehavior, LlmFallbackConfig,
    LlmMetricsConfig, LlmPoolConfig, MetricsConfig, OnAllFailedBehavior, PilotMetricsConfig,
    RetrievalConfig, RetrievalMetricsConfig, RetryConfig, SearchConfig, Severity, StorageConfig,
    StrategyConfig, SufficiencyConfig, SummaryConfig, ThrottleConfig, ValidationError,
};
