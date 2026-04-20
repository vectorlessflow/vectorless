// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Summary generation strategies.

use async_trait::async_trait;

use crate::document::{DocumentTree, NodeId};
use crate::llm::memo::{MemoKey, MemoStore, MemoValue};
use crate::llm::{LlmClient, LlmResult};
use crate::utils::fingerprint::Fingerprint;

/// Configuration for summary strategies.
#[derive(Debug, Clone)]
pub struct SummaryStrategyConfig {
    /// Maximum tokens for a summary.
    pub max_tokens: usize,

    /// Minimum content tokens to generate summary.
    pub min_content_tokens: usize,

    /// Whether to persist lazy-generated summaries.
    pub persist_lazy: bool,

    /// Token threshold below which the original content is used as summary
    /// instead of calling LLM. Saves API cost for short, self-contained nodes.
    /// Set to 0 to always call LLM.
    pub shortcut_threshold: usize,
}

impl Default for SummaryStrategyConfig {
    fn default() -> Self {
        Self {
            max_tokens: 200,
            min_content_tokens: 50,
            persist_lazy: false,
            shortcut_threshold: 50,
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
        Self::Full {
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

    /// Get the shortcut threshold (tokens below which content is used as-is).
    pub fn shortcut_threshold(&self) -> usize {
        self.config().shortcut_threshold
    }
}

/// Summary generator trait.
#[async_trait]
pub trait SummaryGenerator: Send + Sync {
    /// Generate a summary for the given content.
    async fn generate(&self, title: &str, content: &str) -> LlmResult<String>;

    /// Generate a summary with leaf/non-leaf context.
    /// Non-leaf nodes get a navigation-oriented prompt ("what does this section cover"),
    /// leaf nodes get a content-oriented prompt ("what does this section say").
    async fn generate_for_node(
        &self,
        title: &str,
        content: &str,
        is_leaf: bool,
    ) -> LlmResult<String> {
        let _ = is_leaf;
        self.generate(title, content).await
    }
}

/// LLM-based summary generator.
pub struct LlmSummaryGenerator {
    client: LlmClient,
    max_tokens: usize,
    /// Optional memo store for caching results.
    memo_store: Option<MemoStore>,
}

impl LlmSummaryGenerator {
    /// Create a new summary generator.
    pub fn new(client: LlmClient) -> Self {
        Self {
            client,
            max_tokens: 200,
            memo_store: None,
        }
    }

    /// Set max tokens.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set memo store for caching.
    pub fn with_memo_store(mut self, store: MemoStore) -> Self {
        self.memo_store = Some(store);
        self
    }
}

#[async_trait]
impl SummaryGenerator for LlmSummaryGenerator {
    async fn generate(&self, title: &str, content: &str) -> LlmResult<String> {
        // Compute content fingerprint for cache key
        let content_fp = Fingerprint::from_str(&format!("{}|{}", title, content));
        let memo_key = MemoKey::summary(&content_fp);

        // Check memo store first
        if let Some(ref store) = self.memo_store {
            if let Some(cached) = store.get(&memo_key) {
                if let Some(summary) = cached.as_summary() {
                    tracing::debug!("Memo cache hit for summary: {}", title);
                    return Ok(summary.to_string());
                }
            }
        }

        // Generate with LLM
        let system_prompt = "You are a document summarization assistant. \
            Generate a concise summary (2-3 sentences) of the given section. \
            Focus on the main topics and key information. \
            Respond with only the summary, no additional text.";

        let user_prompt = format!("Title: {}\n\nContent:\n{}", title, content);

        let summary = self
            .client
            .complete_with_max_tokens(&system_prompt, &user_prompt, self.max_tokens as u16)
            .await?;

        // Cache the result
        if let Some(ref store) = self.memo_store {
            // Estimate tokens saved (roughly: input + output tokens)
            let tokens_saved = (title.len() + content.len() + summary.len()) / 4;
            store.put_with_tokens(
                memo_key,
                MemoValue::Summary(summary.clone()),
                tokens_saved as u64,
            );
            tracing::debug!("Memo cache stored for summary: {}", title);
        }

        Ok(summary)
    }

    async fn generate_for_node(
        &self,
        title: &str,
        content: &str,
        is_leaf: bool,
    ) -> LlmResult<String> {
        // Compute content fingerprint for cache key (include leaf flag)
        let content_fp = Fingerprint::from_str(&format!("{}|{}|leaf={}", title, content, is_leaf));
        let memo_key = MemoKey::summary(&content_fp);

        // Check memo store first
        if let Some(ref store) = self.memo_store {
            if let Some(cached) = store.get(&memo_key) {
                if let Some(summary) = cached.as_summary() {
                    tracing::debug!("Memo cache hit for summary: {}", title);
                    return Ok(summary.to_string());
                }
            }
        }

        // Choose prompt based on node type
        let system_prompt = if is_leaf {
            // Leaf nodes: content-oriented — "what does this section say"
            "You are a document summarization assistant. \
            Generate a concise summary (2-3 sentences) of the given section's content. \
            Focus on the key information and facts presented. \
            Respond with only the summary, no additional text."
        } else {
            // Non-leaf (branch) nodes: navigation-oriented with structured output.
            // Produces OVERVIEW, QUESTIONS, and TAGS sections that EnhanceStage parses.
            "You are a document navigation assistant. \
            Generate a structured overview of this section for navigation purposes. \
            Respond in EXACTLY this format (one section per line):\n\
            OVERVIEW: <2-3 sentence description of what topics this section covers>\n\
            QUESTIONS: <comma-separated list of 3-5 typical questions this section can answer>\n\
            TAGS: <comma-separated list of 2-4 topic keywords>"
        };

        let user_prompt = if is_leaf {
            format!("Title: {}\n\nContent:\n{}", title, content)
        } else {
            // For non-leaf nodes, include children info for better routing summaries
            format!("Title: {}\n\nContent:\n{}", title, content)
        };

        let summary = self
            .client
            .complete_with_max_tokens(&system_prompt, &user_prompt, self.max_tokens as u16)
            .await?;

        // Cache the result
        if let Some(ref store) = self.memo_store {
            let tokens_saved = (title.len() + content.len() + summary.len()) / 4;
            store.put_with_tokens(
                memo_key,
                MemoValue::Summary(summary.clone()),
                tokens_saved as u64,
            );
            tracing::debug!("Memo cache stored for summary: {}", title);
        }

        Ok(summary)
    }
}
