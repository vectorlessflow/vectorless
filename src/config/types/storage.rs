// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Storage and sufficiency configuration types.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Workspace directory for persisted documents.
    #[serde(default = "default_workspace_dir")]
    pub workspace_dir: PathBuf,

    /// LRU cache size (number of documents).
    #[serde(default = "default_cache_size")]
    pub cache_size: usize,

    /// Enable atomic writes (write to temp file, then rename).
    /// This prevents data corruption on crash.
    #[serde(default = "default_atomic_writes")]
    pub atomic_writes: bool,

    /// Enable file locking for multi-process safety.
    #[serde(default = "default_file_lock")]
    pub file_lock: bool,

    /// Enable checksum verification for data integrity.
    #[serde(default = "default_checksum_enabled")]
    pub checksum_enabled: bool,

    /// Enable compression for stored documents.
    #[serde(default)]
    pub compression: CompressionConfig,
}

fn default_workspace_dir() -> PathBuf {
    PathBuf::from("./workspace")
}

fn default_cache_size() -> usize {
    100
}

fn default_atomic_writes() -> bool {
    true
}

fn default_file_lock() -> bool {
    true
}

fn default_checksum_enabled() -> bool {
    true
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            workspace_dir: default_workspace_dir(),
            cache_size: default_cache_size(),
            atomic_writes: default_atomic_writes(),
            file_lock: default_file_lock(),
            checksum_enabled: default_checksum_enabled(),
            compression: CompressionConfig::default(),
        }
    }
}

impl StorageConfig {
    /// Create new storage config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the workspace directory.
    pub fn with_workspace_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.workspace_dir = dir.into();
        self
    }

    /// Set the cache size.
    pub fn with_cache_size(mut self, size: usize) -> Self {
        self.cache_size = size;
        self
    }

    /// Enable or disable atomic writes.
    pub fn with_atomic_writes(mut self, enabled: bool) -> Self {
        self.atomic_writes = enabled;
        self
    }

    /// Enable or disable file locking.
    pub fn with_file_lock(mut self, enabled: bool) -> Self {
        self.file_lock = enabled;
        self
    }

    /// Enable or disable checksum verification.
    pub fn with_checksum(mut self, enabled: bool) -> Self {
        self.checksum_enabled = enabled;
        self
    }

    /// Set compression configuration.
    pub fn with_compression(mut self, compression: CompressionConfig) -> Self {
        self.compression = compression;
        self
    }
}

/// Compression configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Enable compression.
    #[serde(default = "default_compression_enabled")]
    pub enabled: bool,

    /// Compression algorithm.
    #[serde(default = "default_compression_algorithm")]
    pub algorithm: CompressionAlgorithm,

    /// Compression level (1-9, higher = better compression but slower).
    #[serde(default = "default_compression_level")]
    pub level: u32,
}

fn default_compression_enabled() -> bool {
    false
}

fn default_compression_algorithm() -> CompressionAlgorithm {
    CompressionAlgorithm::Gzip
}

fn default_compression_level() -> u32 {
    6
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: default_compression_enabled(),
            algorithm: default_compression_algorithm(),
            level: default_compression_level(),
        }
    }
}

impl CompressionConfig {
    /// Create new compression config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable compression.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set the compression algorithm.
    pub fn with_algorithm(mut self, algorithm: CompressionAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Set the compression level.
    pub fn with_level(mut self, level: u32) -> Self {
        self.level = level.clamp(1, 9);
        self
    }
}

/// Compression algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionAlgorithm {
    /// Gzip compression.
    Gzip,
    /// Zstandard compression.
    Zstd,
}

/// Sufficiency checker configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SufficiencyConfig {
    /// Minimum tokens for sufficiency.
    #[serde(default = "default_min_tokens")]
    pub min_tokens: usize,

    /// Target tokens for full sufficiency.
    #[serde(default = "default_target_tokens")]
    pub target_tokens: usize,

    /// Maximum tokens before stopping.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Minimum content length (characters).
    #[serde(default = "default_min_content_length")]
    pub min_content_length: usize,

    /// Confidence threshold for LLM judge.
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f32,
}

fn default_min_tokens() -> usize {
    500
}

fn default_target_tokens() -> usize {
    2000
}

fn default_max_tokens() -> usize {
    4000
}

fn default_min_content_length() -> usize {
    200
}

fn default_confidence_threshold() -> f32 {
    0.7
}

impl Default for SufficiencyConfig {
    fn default() -> Self {
        Self {
            min_tokens: default_min_tokens(),
            target_tokens: default_target_tokens(),
            max_tokens: default_max_tokens(),
            min_content_length: default_min_content_length(),
            confidence_threshold: default_confidence_threshold(),
        }
    }
}

impl SufficiencyConfig {
    /// Create new sufficiency config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the minimum tokens.
    pub fn with_min_tokens(mut self, tokens: usize) -> Self {
        self.min_tokens = tokens;
        self
    }

    /// Set the target tokens.
    pub fn with_target_tokens(mut self, tokens: usize) -> Self {
        self.target_tokens = tokens;
        self
    }

    /// Set the maximum tokens.
    pub fn with_max_tokens(mut self, tokens: usize) -> Self {
        self.max_tokens = tokens;
        self
    }

    /// Set the confidence threshold.
    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold;
        self
    }
}

/// Cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum number of cache entries.
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,

    /// Time-to-live for cache entries (seconds).
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
}

fn default_max_entries() -> usize {
    1000
}

fn default_ttl_secs() -> u64 {
    3600
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: default_max_entries(),
            ttl_secs: default_ttl_secs(),
        }
    }
}

impl CacheConfig {
    /// Create new cache config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum entries.
    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = max;
        self
    }

    /// Set the TTL in seconds.
    pub fn with_ttl_secs(mut self, secs: u64) -> Self {
        self.ttl_secs = secs;
        self
    }
}

/// Strategy-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// MCTS exploration weight (sqrt(2) ≈ 1.414).
    #[serde(default = "default_exploration_weight")]
    pub exploration_weight: f32,

    /// Semantic similarity threshold.
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,

    /// High similarity threshold for "answer" decision.
    #[serde(default = "default_high_similarity_threshold")]
    pub high_similarity_threshold: f32,

    /// Low similarity threshold for "explore" decision.
    #[serde(default = "default_low_similarity_threshold")]
    pub low_similarity_threshold: f32,
}

fn default_exploration_weight() -> f32 {
    1.414
}

fn default_similarity_threshold() -> f32 {
    0.5
}

fn default_high_similarity_threshold() -> f32 {
    0.8
}

fn default_low_similarity_threshold() -> f32 {
    0.3
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            exploration_weight: default_exploration_weight(),
            similarity_threshold: default_similarity_threshold(),
            high_similarity_threshold: default_high_similarity_threshold(),
            low_similarity_threshold: default_low_similarity_threshold(),
        }
    }
}

impl StrategyConfig {
    /// Create new strategy config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the exploration weight.
    pub fn with_exploration_weight(mut self, weight: f32) -> Self {
        self.exploration_weight = weight;
        self
    }

    /// Set the similarity threshold.
    pub fn with_similarity_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_config_defaults() {
        let config = StorageConfig::default();
        assert_eq!(config.workspace_dir, PathBuf::from("./workspace"));
        assert_eq!(config.cache_size, 100);
        assert!(config.atomic_writes);
        assert!(config.file_lock);
        assert!(config.checksum_enabled);
        assert!(!config.compression.enabled);
    }

    #[test]
    fn test_storage_config_builders() {
        let config = StorageConfig::new()
            .with_workspace_dir("/data/workspace")
            .with_cache_size(200)
            .with_atomic_writes(false)
            .with_file_lock(false)
            .with_checksum(false);

        assert_eq!(config.workspace_dir, PathBuf::from("/data/workspace"));
        assert_eq!(config.cache_size, 200);
        assert!(!config.atomic_writes);
        assert!(!config.file_lock);
        assert!(!config.checksum_enabled);
    }

    #[test]
    fn test_compression_config_defaults() {
        let config = CompressionConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.algorithm, CompressionAlgorithm::Gzip);
        assert_eq!(config.level, 6);
    }

    #[test]
    fn test_compression_config_level_clamp() {
        let config = CompressionConfig::new().with_level(15);
        assert_eq!(config.level, 9); // clamped to max

        let config = CompressionConfig::new().with_level(0);
        assert_eq!(config.level, 1); // clamped to min
    }

    #[test]
    fn test_sufficiency_config_defaults() {
        let config = SufficiencyConfig::default();
        assert_eq!(config.min_tokens, 500);
        assert_eq!(config.target_tokens, 2000);
        assert_eq!(config.max_tokens, 4000);
    }

    #[test]
    fn test_cache_config_defaults() {
        let config = CacheConfig::default();
        assert_eq!(config.max_entries, 1000);
        assert_eq!(config.ttl_secs, 3600);
    }

    #[test]
    fn test_strategy_config_defaults() {
        let config = StrategyConfig::default();
        assert!((config.exploration_weight - 1.414).abs() < 0.001);
        assert_eq!(config.similarity_threshold, 0.5);
    }
}
