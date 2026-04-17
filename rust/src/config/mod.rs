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

pub use types::Config;
pub(crate) use types::{
    CacheConfig, CompressionAlgorithm, ContentAggregatorConfig, FallbackBehavior, FallbackConfig,
    IndexerConfig, LlmConfig, LlmMetricsConfig, MetricsConfig, OnAllFailedBehavior,
    PilotMetricsConfig, RetrievalConfig, RetrievalMetricsConfig, RetryConfig, SlotConfig,
    StrategyConfig, SufficiencyConfig, ThrottleConfig,
};
