// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Summary generation strategies.

use async_trait::async_trait;

use crate::document::{DocumentTree, NodeId};
use crate::llm::{LlmClient, LlmResult};

/// Configuration for summary strategies.
#[derive(Debug, Clone)]
pub struct SummaryStrategyConfig {
    /// Maximum tokens for a summary.
    pub max_tokens: usize,

    /// Minimum content tokens to generate summary.
    pub min_content_tokens: usize,

    /// Whether to persist lazy-generated summaries.
    pub persist_lazy: bool,
}

impl Default for SummaryStrategyConfig {
    fn default() -> Self {
        Self {
            max_tokens: 200,
            min_content_tokens: 50,
            persist_lazy: false,
        }
    }
}

/// Strategy for generating summaries.
#[derive(Debug, Clone)]
pub enum SummaryStrategy {
    /// No summary generation.
    None,

    /// Generate for all nodes.
    Full {
        /// Strategy configuration.
        config: SummaryStrategyConfig,
    },

    /// Generate selectively.
    Selective {
        /// Minimum tokens threshold.
        min_tokens: usize,

        /// Only generate for branch nodes (non-leaves).
        branch_only: bool,

        /// Strategy configuration.
        config: SummaryStrategyConfig,
    },

    /// Generate on-demand at query time.
    Lazy {
        /// Whether to persist generated summaries.
        persist: bool,

        /// Strategy configuration.
        config: SummaryStrategyConfig,
    },
}

impl Default for SummaryStrategy {
    fn default() -> Self {
        Self::Selective {
            min_tokens: 100,
            branch_only: true,
            config: SummaryStrategyConfig::default(),
        }
    }
}

impl SummaryStrategy {
    /// Create a "none" strategy.
    pub fn none() -> Self {
        Self::None
    }

    /// Create a "full" strategy.
    pub fn full() -> Self {
        Self::Full {
            config: SummaryStrategyConfig::default(),
        }
    }

    /// Create a "selective" strategy.
    pub fn selective(min_tokens: usize, branch_only: bool) -> Self {
        Self::Selective {
            min_tokens,
            branch_only,
            config: SummaryStrategyConfig::default(),
        }
    }

    /// Create a "lazy" strategy.
    pub fn lazy(persist: bool) -> Self {
        Self::Lazy {
            persist,
            config: SummaryStrategyConfig::default(),
        }
    }

    /// Check if we should generate a summary for a node.
    pub fn should_generate(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        token_count: usize,
    ) -> bool {
        match self {
            Self::None => false,
            Self::Full { .. } => token_count > 0,
            Self::Selective {
                min_tokens,
                branch_only,
                ..
            } => {
                let is_branch = !tree.is_leaf(node_id);
                let enough_tokens = token_count >= *min_tokens;

                if *branch_only {
                    is_branch && enough_tokens
                } else {
                    enough_tokens
                }
            }
            Self::Lazy { .. } => false, // Generated on-demand
        }
    }

    /// Check if lazy strategy is enabled.
    pub fn is_lazy(&self) -> bool {
        matches!(self, Self::Lazy { .. })
    }

    /// Get the config.
    pub fn config(&self) -> SummaryStrategyConfig {
        match self {
            Self::None => SummaryStrategyConfig::default(),
            Self::Full { config } => config.clone(),
            Self::Selective { config, .. } => config.clone(),
            Self::Lazy { config, .. } => config.clone(),
        }
    }
}

/// Summary generator trait.
#[async_trait]
pub trait SummaryGenerator: Send + Sync {
    /// Generate a summary for the given content.
    async fn generate(&self, title: &str, content: &str) -> LlmResult<String>;
}

/// LLM-based summary generator.
pub struct LlmSummaryGenerator {
    client: LlmClient,
    max_tokens: usize,
}

impl LlmSummaryGenerator {
    /// Create a new summary generator.
    pub fn new(client: LlmClient) -> Self {
        Self {
            client,
            max_tokens: 200,
        }
    }

    /// Set max tokens.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }
}

#[async_trait]
impl SummaryGenerator for LlmSummaryGenerator {
    async fn generate(&self, title: &str, content: &str) -> LlmResult<String> {
        let system_prompt = "You are a document summarization assistant. \
            Generate a concise summary (2-3 sentences) of the given section. \
            Focus on the main topics and key information. \
            Respond with only the summary, no additional text.";

        let user_prompt = format!("Title: {}\n\nContent:\n{}", title, content);

        self.client
            .complete_with_max_tokens(&system_prompt, &user_prompt, self.max_tokens as u16)
            .await
    }
}
