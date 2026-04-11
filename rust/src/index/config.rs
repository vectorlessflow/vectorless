// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration types for the index pipeline.
//!
//! This module contains all configuration types used by the indexing pipeline:
//! - [`IndexMode`] - Document format selection
//! - [`PipelineOptions`] - Full pipeline configuration
//! - [`OptimizationConfig`] - Tree optimization settings
//! - [`ThinningConfig`] - Node merging settings

use super::summary::SummaryStrategy;
use crate::config::{ConcurrencyConfig, IndexerConfig};
use crate::document::{DocumentTree, ReasoningIndexConfig};
use crate::utils::fingerprint::{Fingerprint, Fingerprinter};

/// Index mode for document processing.
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

impl OptimizationConfig {
    /// Create a new optimization config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable optimization entirely.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    /// Set maximum depth.
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Set maximum children per node.
    pub fn with_max_children(mut self, max: usize) -> Self {
        self.max_children = Some(max);
        self
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

    /// Set the token threshold.
    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold = threshold;
        self
    }
}

/// Pipeline options for index execution.
#[derive(Debug, Clone)]
pub struct PipelineOptions {
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

    /// Reasoning index configuration.
    pub reasoning_index: ReasoningIndexConfig,

    /// Existing tree from a previous index (for incremental updates).
    /// Stages (enhance, reasoning) can reuse data from unchanged nodes.
    pub existing_tree: Option<DocumentTree>,

    /// Current processing version. Bumped when indexing algorithm changes
    /// to force reprocessing of existing documents.
    pub processing_version: u32,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            mode: IndexMode::Auto,
            generate_ids: true,
            summary_strategy: SummaryStrategy::full(),
            thinning: ThinningConfig::default(),
            optimization: OptimizationConfig::default(),
            generate_description: true,
            concurrency: ConcurrencyConfig::default(),
            indexer: IndexerConfig::default(),
            reasoning_index: ReasoningIndexConfig::default(),
            existing_tree: None,
            processing_version: 1,
        }
    }
}

impl PipelineOptions {
    /// Create new pipeline options with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the index mode.
    pub fn with_mode(mut self, mode: IndexMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set whether to generate node IDs.
    pub fn with_generate_ids(mut self, generate: bool) -> Self {
        self.generate_ids = generate;
        self
    }

    /// Set the summary strategy.
    pub fn with_summary_strategy(mut self, strategy: SummaryStrategy) -> Self {
        self.summary_strategy = strategy;
        self
    }

    /// Set the thinning configuration.
    pub fn with_thinning(mut self, thinning: ThinningConfig) -> Self {
        self.thinning = thinning;
        self
    }

    /// Set the optimization configuration.
    pub fn with_optimization(mut self, optimization: OptimizationConfig) -> Self {
        self.optimization = optimization;
        self
    }

    /// Set whether to generate document description.
    pub fn with_generate_description(mut self, generate: bool) -> Self {
        self.generate_description = generate;
        self
    }

    /// Set the concurrency configuration.
    pub fn with_concurrency(mut self, concurrency: ConcurrencyConfig) -> Self {
        self.concurrency = concurrency;
        self
    }

    /// Set the indexer configuration.
    pub fn with_indexer(mut self, indexer: IndexerConfig) -> Self {
        self.indexer = indexer;
        self
    }

    /// Set the reasoning index configuration.
    pub fn with_reasoning_index(mut self, config: ReasoningIndexConfig) -> Self {
        self.reasoning_index = config;
        self
    }

    /// Compute a fingerprint of the pipeline configuration.
    ///
    /// If this fingerprint changes between runs, all documents need full reprocessing
    /// even if their content hasn't changed (because the processing logic is different).
    pub fn logic_fingerprint(&self) -> Fingerprint {
        Fingerprinter::new()
            .with_str(&format!("{:?}", self.mode))
            .with_bool(self.generate_ids)
            .with_str(&format!("{:?}", self.summary_strategy))
            .with_bool(self.generate_description)
            .with_bool(self.optimization.enabled)
            .with_str(&format!("{:?}", self.reasoning_index))
            .into_fingerprint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_mode_default() {
        let mode = IndexMode::default();
        assert_eq!(mode, IndexMode::Auto);
    }

    #[test]
    fn test_optimization_config() {
        let config = OptimizationConfig::new()
            .with_max_depth(5)
            .with_max_children(10);

        assert!(config.enabled);
        assert_eq!(config.max_depth, Some(5));
        assert_eq!(config.max_children, Some(10));
    }

    #[test]
    fn test_thinning_config() {
        let config = ThinningConfig::enabled(300);
        assert!(config.enabled);
        assert_eq!(config.threshold, 300);

        let disabled = ThinningConfig::disabled();
        assert!(!disabled.enabled);
    }

    #[test]
    fn test_pipeline_options_builder() {
        let options = PipelineOptions::new()
            .with_mode(IndexMode::Markdown)
            .with_generate_ids(false);

        assert_eq!(options.mode, IndexMode::Markdown);
        assert!(!options.generate_ids);
    }
}
