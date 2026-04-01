// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration type definitions.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration for vectorless.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Indexer configuration.
    #[serde(default)]
    pub indexer: IndexerConfig,

    /// Summary model configuration.
    #[serde(default)]
    pub summary: SummaryConfig,

    /// Retrieval model configuration.
    #[serde(default)]
    pub retrieval: RetrievalConfig,

    /// Storage configuration.
    #[serde(default)]
    pub storage: StorageConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            indexer: IndexerConfig::default(),
            summary: SummaryConfig::default(),
            retrieval: RetrievalConfig::default(),
            storage: StorageConfig::default(),
        }
    }
}

/// Indexer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerConfig {
    /// Word count threshold for splitting sections into subsections.
    #[serde(default = "default_subsection_threshold")]
    pub subsection_threshold: usize,

    /// Maximum tokens to send in a single segmentation request.
    #[serde(default = "default_max_segment_tokens")]
    pub max_segment_tokens: usize,

    /// Maximum tokens for each summary.
    #[serde(default = "default_max_summary_tokens")]
    pub max_summary_tokens: usize,
}

pub fn default_subsection_threshold() -> usize {
    300
}

pub fn default_max_segment_tokens() -> usize {
    3000
}

pub fn default_max_summary_tokens() -> usize {
    200
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            subsection_threshold: default_subsection_threshold(),
            max_segment_tokens: default_max_segment_tokens(),
            max_summary_tokens: default_max_summary_tokens(),
        }
    }
}

/// Summary model configuration (for indexing/summarization).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryConfig {
    /// Model name for summarization.
    #[serde(default = "default_summary_model")]
    pub model: String,

    /// API endpoint for summary model.
    #[serde(default = "default_summary_endpoint")]
    pub endpoint: String,

    /// API key (prefer using environment variable).
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for summary generation.
    #[serde(default = "default_summary_max_tokens")]
    pub max_tokens: usize,

    /// Temperature for summary generation.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

pub fn default_summary_model() -> String {
    // Auto-detect based on available API keys
    // if std::env::var("OPENAI_API_KEY").is_ok() {
    //     "gpt-4o-mini".to_string()
    // } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
    //     "claude-3-haiku-20240307".to_string()
    // } else {
    //     "glm-5".to_string()
    // }

    "glm-5".to_string()
}

pub fn default_summary_endpoint() -> String {
    // Auto-detect based on available API keys
    if std::env::var("OPENAI_API_KEY").is_ok() {
        "https://api.openai.com/v1".to_string()
    } else if std::env::var("AZURE_OPENAI_ENDPOINT").is_ok() {
        std::env::var("AZURE_OPENAI_ENDPOINT").unwrap_or_default()
    } else {
        "https://api.z.ai/api/coding/paas/v4".to_string()
    }
}

pub fn default_summary_max_tokens() -> usize {
    200
}

pub fn default_temperature() -> f32 {
    0.0
}

impl Default for SummaryConfig {
    fn default() -> Self {
        Self {
            model: default_summary_model(),
            endpoint: default_summary_endpoint(),
            api_key: None,
            max_tokens: default_summary_max_tokens(),
            temperature: default_temperature(),
        }
    }
}

/// Retriever type enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrieverType {
    /// LLM-based tree navigation (default).
    LlmNavigate,
    /// Beam search traversal.
    BeamSearch,
    /// Monte Carlo Tree Search.
    Mcsts,
    /// Multi-document retrieval.
    MultiDoc,
    /// Hybrid approach combining multiple strategies.
    Hybrid,
}

impl Default for RetrieverType {
    fn default() -> Self {
        Self::LlmNavigate
    }
}

/// Retrieval model configuration (for navigation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalConfig {
    /// Model name for retrieval/navigation.
    #[serde(default = "default_retrieval_model")]
    pub model: String,

    /// API endpoint for retrieval model.
    #[serde(default = "default_retrieval_endpoint")]
    pub endpoint: String,

    /// API key (prefer using environment variable).
    #[serde(default)]
    pub api_key: Option<String>,

    /// Maximum tokens for retrieval context.
    #[serde(default = "default_retrieval_max_tokens")]
    pub max_tokens: usize,

    /// Temperature for retrieval.
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Number of top-k results to return.
    #[serde(default = "default_top_k")]
    pub top_k: usize,

    /// Retriever type to use.
    #[serde(default)]
    pub retriever_type: RetrieverType,
}

pub fn default_retrieval_model() -> String {
    // Auto-detect based on available API keys
    if std::env::var("OPENAI_API_KEY").is_ok() {
        "gpt-4o".to_string()
    } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        "claude-3-sonnet-20240229".to_string()
    } else {
        "glm-5".to_string()
    }
}

pub fn default_retrieval_endpoint() -> String {
    // Auto-detect based on available API keys
    if std::env::var("OPENAI_API_KEY").is_ok() {
        "https://api.openai.com/v1".to_string()
    } else if std::env::var("AZURE_OPENAI_ENDPOINT").is_ok() {
        std::env::var("AZURE_OPENAI_ENDPOINT").unwrap_or_default()
    } else {
        "https://api.z.ai/api/paas/v4".to_string()
    }
}

pub fn default_retrieval_max_tokens() -> usize {
    1000
}

pub fn default_top_k() -> usize {
    3
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            model: default_retrieval_model(),
            endpoint: default_retrieval_endpoint(),
            api_key: None,
            max_tokens: default_retrieval_max_tokens(),
            temperature: default_temperature(),
            top_k: default_top_k(),
            retriever_type: RetrieverType::default(),
        }
    }
}

/// Storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Workspace directory for persisted documents.
    ///
    /// Structure:
    /// ```text
    /// workspace/
    /// ├── _meta.json           # Lightweight index
    /// ├── {doc_id_1}.json      # Document 1
    /// └── {doc_id_2}.json      # Document 2
    /// ```
    #[serde(default = "default_workspace_dir")]
    pub workspace_dir: PathBuf,
}

pub fn default_workspace_dir() -> PathBuf {
    PathBuf::from("./workspace")
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            workspace_dir: default_workspace_dir(),
        }
    }
}
