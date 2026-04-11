// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index Pipeline module.
//!
//! This module provides a modular, extensible document indexing pipeline.
//!
//! # Architecture
//!
//! ```text
//! Priority  10: ┌──────────┐
//!               │  Parse    │  Parse document into raw nodes
//!               └────┬─────┘
//! Priority  20: ┌────▼─────┐
//!               │  Build   │  Construct tree + thinning (with content merge)
//!               └────┬─────┘
//! Priority  22: ┌────▼─────┐
//!               │ Validate │  Tree integrity checks (optional)
//!               └────┬─────┘
//! Priority  25: ┌────▼─────┐
//!               │  Split   │  Split oversized leaf nodes (optional)
//!               └────┬─────┘
//! Priority  30: ┌────▼─────┐
//!               │ Enhance  │  LLM summaries (when client available)
//!               └────┬─────┘
//! Priority  40: ┌────▼─────┐
//!               │  Enrich  │  Metadata + cross-references
//!               └────┬─────┘
//! Priority  45: ┌────▼──────────┐
//!               │ Reasoning Idx │  Pre-computed reasoning index
//!               └────┬──────────┘
//! Priority  60: ┌────▼──────┐
//!               │ Optimize  │  Final tree optimization
//!               └───────────┘
//! ```
//!
//! Checkpointing is available when `PipelineOptions::checkpoint_dir` is set.
//! State is saved after each stage group and resumed on restart.
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
pub mod incremental;
pub mod parse;
pub mod pipeline;
pub mod stages;
pub mod summary;

// Re-export main types from pipeline
pub use pipeline::{IndexInput, IndexMetrics, PipelineExecutor, PipelineResult};

// Re-export config types
pub use config::{IndexMode, PipelineOptions, ThinningConfig};

// Re-export summary
pub use summary::SummaryStrategy;
