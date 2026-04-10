// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index Pipeline module.
//!
//! This module provides a modular, extensible document indexing pipeline.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
//! │   Parse     │───►│   Build     │───►│  Enhance    │───►│   Enrich    │
//! │  (Document) │    │   (Tree)    │    │  (LLM Boost)│    │  (Metadata) │
//! └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
//!                                                                  │
//!                                                                  ▼
//! ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
//! │   Output    │◄───│   Persist   │◄───│   Optimize  │◄───│   Enrich    │
//! │  (Indexed)  │    │  (Storage)  │    │   (Tree)    │    │             │
//! └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use vectorless::index::{PipelineExecutor, IndexInput, PipelineOptions};
//! use vectorless::index::summary::SummaryStrategy;
//!
//! let options = PipelineOptions::new()
//!     .with_summary_strategy(SummaryStrategy::selective(100, true));
//!
//! let result = PipelineExecutor::new()
//!     .with_options(options)
//!     .execute(input)
//!     .await?;
//! ```

pub mod config;
pub mod graph_builder;
pub mod incremental;
pub mod pipeline;
pub mod stages;
pub mod summary;

// Re-export main types from pipeline
pub use pipeline::{
    ExecutionGroup, FailurePolicy, IndexContext, IndexInput, IndexMetrics, IndexResult,
    PipelineExecutor, PipelineOrchestrator, StageResult, StageRetryConfig,
};

// Re-export config types
pub use config::{IndexMode, OptimizationConfig, PipelineOptions, ThinningConfig};

// Re-export stages
pub use stages::IndexStage;

// Re-export summary
pub use summary::{
    FullStrategy, LazyStrategy, LlmSummaryGenerator, SelectiveStrategy, SummaryGenerator,
    SummaryStrategy, SummaryStrategyConfig,
};

// Re-export incremental
pub use incremental::{ChangeDetector, ChangeSet, PartialUpdater};

// Re-export config types from crate config
pub use crate::config::{ConcurrencyConfig, IndexerConfig};
