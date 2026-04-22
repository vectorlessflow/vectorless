// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Full summary strategy - generate summaries for all nodes.

use crate::document::NodeId;
use crate::llm::LlmClient;

use super::{SummaryGenerator, SummaryStrategyConfig};

/// Full summary strategy - generates summaries for all nodes.
pub struct FullStrategy {
    /// Summary generator.
    generator: Box<dyn SummaryGenerator>,
    /// Configuration.
    config: SummaryStrategyConfig,
}

impl FullStrategy {
    /// Create a new full strategy with LLM client.
    pub fn new(client: LlmClient) -> Self {
        Self {
            generator: Box::new(super::LlmSummaryGenerator::new(client)),
            config: SummaryStrategyConfig::default(),
        }
    }

    /// Create with custom generator.
    pub fn with_generator(generator: Box<dyn SummaryGenerator>) -> Self {
        Self {
            generator,
            config: SummaryStrategyConfig::default(),
        }
    }

    /// Set configuration.
    pub fn with_config(mut self, config: SummaryStrategyConfig) -> Self {
        self.config = config;
        self
    }

    /// Check if a node should have a summary generated.
    pub fn should_generate(&self, _node_id: NodeId, content_tokens: usize) -> bool {
        // In full mode, generate for all nodes with content
        content_tokens >= self.config.min_content_tokens
    }

    /// Generate a summary for content.
    pub async fn generate(&self, title: &str, content: &str) -> crate::llm::LlmResult<String> {
        self.generator.generate(title, content).await
    }

    /// Get the configuration.
    pub fn config(&self) -> &SummaryStrategyConfig {
        &self.config
    }
}

impl std::fmt::Debug for FullStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FullStrategy")
            .field("config", &self.config)
            .finish()
    }
}
