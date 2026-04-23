// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Internal configuration management.
//!
//! Users configure vectorless via [`EngineBuilder`](vectorless_engine::EngineBuilder) methods,
//! not by directly interacting with this module.

mod types;
mod validator;

pub use types::Config;
pub use types::DocumentGraphConfig;
pub use types::MetricsConfig;
pub use types::LlmMetricsConfig;
pub use types::RetrievalMetricsConfig;
pub use types::{
    CompressionAlgorithm, FallbackBehavior, FallbackConfig, IndexerConfig, LlmConfig,
    OnAllFailedBehavior, RetrievalConfig, RetryConfig, SlotConfig, StorageConfig,
    ThrottleConfig,
};
