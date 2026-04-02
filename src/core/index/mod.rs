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
//! use vectorless::core::index::pipeline::{PipelineExecutor, IndexOptions};
//! use vectorless::core::index::summary::SummaryStrategy;
//!
//! let options = IndexOptions {
//!     summary_strategy: SummaryStrategy::selective(100, true),
//!     ..Default::default()
//! };
//!
//! let result = PipelineExecutor::new()
//!     .with_options(options)
//!     .execute(input)
//!     .await?;
//! ```

pub mod incremental;
pub mod pipeline;
pub mod stages;
pub mod summary;

// Re-export main types from pipeline
pub use pipeline::{IndexContext, IndexInput, IndexMetrics, IndexResult, PipelineExecutor, StageResult};

// Re-export stages
pub use stages::IndexStage;

// Re-export summary
pub use summary::{SummaryStrategy, SummaryStrategyConfig, SummaryGenerator, LlmSummaryGenerator};

// Re-export incremental
pub use incremental::{ChangeDetector, ChangeSet, PartialUpdater};

// Configuration types
use crate::config::{ConcurrencyConfig, IndexerConfig};

/// Configuration for tree optimization.
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    /// Whether optimization is enabled.
    pub enabled: bool,

    /// Maximum tree depth (flatten if exceeded).
    pub max_depth: Option<usize>,

    /// Maximum children per node (group if exceeded).
    pub max_children: Option<usize>,

    /// Minimum tokens for a leaf node (merge smaller ones).
    pub merge_leaf_threshold: usize,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_depth: None,
            max_children: None,
            merge_leaf_threshold: 50,
        }
    }
}

/// Configuration for thinning (merging small nodes).
#[derive(Debug, Clone)]
pub struct ThinningConfig {
    /// Whether thinning is enabled.
    pub enabled: bool,

    /// Token threshold for merging.
    pub threshold: usize,
}

impl Default for ThinningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: 500,
        }
    }
}

impl ThinningConfig {
    /// Create disabled config.
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Create enabled config with threshold.
    pub fn enabled(threshold: usize) -> Self {
        Self {
            enabled: true,
            threshold,
        }
    }
}

/// Index mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexMode {
    /// Auto-detect format from file extension.
    Auto,
    /// Force Markdown format.
    Markdown,
    /// Force PDF format.
    Pdf,
    /// Force DOCX format.
    Docx,
    /// Force HTML format.
    Html,
}

impl Default for IndexMode {
    fn default() -> Self {
        Self::Auto
    }
}

/// Index options (v2).
#[derive(Debug, Clone)]
pub struct IndexOptions {
    /// Index mode.
    pub mode: IndexMode,

    /// Whether to generate node IDs.
    pub generate_ids: bool,

    /// Summary generation strategy.
    pub summary_strategy: SummaryStrategy,

    /// Thinning configuration.
    pub thinning: ThinningConfig,

    /// Optimization configuration.
    pub optimization: OptimizationConfig,

    /// Whether to generate document description.
    pub generate_description: bool,

    /// Concurrency configuration.
    pub concurrency: ConcurrencyConfig,

    /// Indexer configuration.
    pub indexer: IndexerConfig,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            mode: IndexMode::Auto,
            generate_ids: true,
            summary_strategy: SummaryStrategy::default(),
            thinning: ThinningConfig::default(),
            optimization: OptimizationConfig::default(),
            generate_description: true,
            concurrency: ConcurrencyConfig::default(),
            indexer: IndexerConfig::default(),
        }
    }
}
