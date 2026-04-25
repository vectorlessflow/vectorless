// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Storage configuration types.

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

    /// Directory for pipeline checkpoints (derived from `workspace_dir`).
    #[serde(skip)]
    pub checkpoint_dir: PathBuf,
}

fn default_workspace_dir() -> PathBuf {
    default_workspace_path_for_cwd()
}

/// Compute the default workspace path for the current working directory.
///
/// Returns a platform-appropriate path:
/// - **Linux/macOS**: `~/.vectorless/workspaces/{cwd_hash}/`
/// - **Windows**: `%APPDATA%\vectorless\workspaces\{cwd_hash}\`
///
/// where `cwd_hash` is a 12-hex-char hash derived from the current working
/// directory. This ensures different projects automatically get isolated
/// workspaces.
///
/// # Environment variable resolution order
///
/// | Platform | Primary         | Fallback            | Last resort |
/// |----------|-----------------|---------------------|-------------|
/// | Unix     | `$HOME`         | —                   | `"."`       |
/// | Windows  | `%LOCALAPPDATA%`| `%APPDATA%`         | `"."`       |
pub fn default_workspace_path_for_cwd() -> PathBuf {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let base_dir = if cfg!(windows) {
        // Windows: prefer %LOCALAPPDATA% (e.g. C:\Users\xxx\AppData\Local)
        // then %APPDATA% (e.g. C:\Users\xxx\AppData\Roaming)
        std::env::var("LOCALAPPDATA")
            .or_else(|_| std::env::var("APPDATA"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else {
        // Unix (Linux, macOS): use $HOME
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    };

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let mut hasher = DefaultHasher::new();
    cwd.to_string_lossy().hash(&mut hasher);
    let hash = format!("{:012x}", hasher.finish());

    base_dir.join(".vectorless").join("workspaces").join(hash)
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
        let workspace_dir = default_workspace_dir();
        let checkpoint_dir = workspace_dir.join("checkpoints");
        Self {
            workspace_dir,
            cache_size: default_cache_size(),
            atomic_writes: default_atomic_writes(),
            file_lock: default_file_lock(),
            checksum_enabled: default_checksum_enabled(),
            compression: CompressionConfig::default(),
            checkpoint_dir,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_config_defaults() {
        let config = StorageConfig::default();
        let path_str = config.workspace_dir.to_string_lossy();
        if cfg!(windows) {
            assert!(
                path_str.contains("vectorless"),
                "expected ...\\vectorless\\workspaces\\..., got {:?}",
                config.workspace_dir,
            );
        } else {
            assert!(
                path_str.contains(".vectorless"),
                "expected ~/.vectorless/workspaces/..., got {:?}",
                config.workspace_dir,
            );
        }
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
        assert_eq!(config.level, 9);

        let config = CompressionConfig::new().with_level(0);
        assert_eq!(config.level, 1);
    }
}
