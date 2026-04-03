// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Selective summary strategy - generate summaries only for qualifying nodes.

use crate::domain::{NodeId, DocumentTree};
use crate::llm::LlmClient;

use super::{SummaryGenerator, SummaryStrategyConfig};

/// Selective summary strategy - generates summaries only for nodes that meet criteria.
pub struct SelectiveStrategy {
    /// Summary generator.
    generator: Box<dyn SummaryGenerator>,
    /// Minimum token threshold.
    min_tokens: usize,
    /// Only generate for branch nodes (non-leaves).
    branch_only: bool,
    /// Configuration.
    config: SummaryStrategyConfig,
}

impl SelectiveStrategy {
    /// Create a new selective strategy with default settings.
    pub fn new(client: LlmClient) -> Self {
        Self {
            generator: Box::new(super::LlmSummaryGenerator::new(client)),
            min_tokens: 100,
            branch_only: true,
            config: SummaryStrategyConfig::default(),
        }
    }

    /// Create with custom thresholds.
    pub fn with_thresholds(client: LlmClient, min_tokens: usize, branch_only: bool) -> Self {
        Self {
            generator: Box::new(super::LlmSummaryGenerator::new(client)),
            min_tokens,
            branch_only,
            config: SummaryStrategyConfig::default(),
        }
    }

    /// Create with custom generator.
    pub fn with_generator(generator: Box<dyn SummaryGenerator>) -> Self {
        Self {
            generator,
            min_tokens: 100,
            branch_only: true,
            config: SummaryStrategyConfig::default(),
        }
    }

    /// Set minimum token threshold.
    pub fn with_min_tokens(mut self, min_tokens: usize) -> Self {
        self.min_tokens = min_tokens;
        self
    }

    /// Set branch-only mode.
    pub fn with_branch_only(mut self, branch_only: bool) -> Self {
        self.branch_only = branch_only;
        self
    }

    /// Set configuration.
    pub fn with_config(mut self, config: SummaryStrategyConfig) -> Self {
        self.config = config;
        self
    }

    /// Check if a node should have a summary generated.
    pub fn should_generate(&self, tree: &DocumentTree, node_id: NodeId, token_count: usize) -> bool {
        // Check token threshold
        let enough_tokens = token_count >= self.min_tokens;

        // Check if branch-only
        if self.branch_only {
            let is_branch = !tree.is_leaf(node_id);
            is_branch && enough_tokens
        } else {
            enough_tokens
        }
    }

    /// Generate a summary for content.
    pub async fn generate(&self, title: &str, content: &str) -> crate::llm::LlmResult<String> {
        self.generator.generate(title, content).await
    }

    /// Get the minimum token threshold.
    pub fn min_tokens(&self) -> usize {
        self.min_tokens
    }

    /// Check if branch-only mode is enabled.
    pub fn is_branch_only(&self) -> bool {
        self.branch_only
    }

    /// Get the configuration.
    pub fn config(&self) -> &SummaryStrategyConfig {
        &self.config
    }
}

impl std::fmt::Debug for SelectiveStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelectiveStrategy")
            .field("min_tokens", &self.min_tokens)
            .field("branch_only", &self.branch_only)
            .field("config", &self.config)
            .finish()
    }
}
