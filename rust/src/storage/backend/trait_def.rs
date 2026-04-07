// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Storage backend trait definition.

use std::fmt::Debug;

use crate::error::Result;

/// Storage backend trait for abstracting different storage systems.
///
/// This trait provides a simple key-value interface for document storage.
/// Implementations can use different underlying storage systems:
///
/// - File system
/// - In-memory (for testing)
/// - Database (SQLite, RocksDB, etc.)
/// - Cloud storage (S3, etc.)
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to support concurrent access.
pub trait StorageBackend: Debug + Send + Sync {
    /// Get a value by key.
    ///
    /// Returns `None` if the key doesn't exist.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Store a value with the given key.
    ///
    /// Overwrites any existing value.
    fn put(&self, key: &str, value: &[u8]) -> Result<()>;

    /// Delete a value by key.
    ///
    /// Returns `true` if the value was deleted, `false` if it didn't exist.
    fn delete(&self, key: &str) -> Result<bool>;

    /// Check if a key exists.
    fn exists(&self, key: &str) -> Result<bool>;

    /// List all keys in the storage.
    fn keys(&self) -> Result<Vec<String>>;

    /// Get the number of entries in storage.
    fn len(&self) -> Result<usize>;

    /// Check if storage is empty.
    fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Clear all entries from storage.
    fn clear(&self) -> Result<()>;

    // ========================================================================
    // Batch operations (optional, default implementations)
    // ========================================================================

    /// Get multiple values by keys.
    ///
    /// Returns a vector of options, one for each key.
    fn batch_get(&self, keys: &[&str]) -> Result<Vec<Option<Vec<u8>>>> {
        keys.iter().map(|k| self.get(k)).collect()
    }

    /// Store multiple key-value pairs.
    ///
    /// Default implementation calls `put` for each item.
    fn batch_put(&self, items: &[(&str, &[u8])]) -> Result<()> {
        for (key, value) in items {
            self.put(key, value)?;
        }
        Ok(())
    }

    /// Delete multiple keys.
    ///
    /// Returns the number of keys that were actually deleted.
    fn batch_delete(&self, keys: &[&str]) -> Result<usize> {
        let mut count = 0;
        for key in keys {
            if self.delete(key)? {
                count += 1;
            }
        }
        Ok(count)
    }

    // ========================================================================
    // Metadata operations
    // ========================================================================

    /// Get storage backend name.
    fn backend_name(&self) -> &'static str;

    /// Get storage statistics.
    fn stats(&self) -> StorageStats {
        StorageStats {
            backend: self.backend_name().to_string(),
            entries: self.len().unwrap_or(0),
        }
    }
}

/// Storage statistics.
#[derive(Debug, Clone)]
pub struct StorageStats {
    /// Backend name.
    pub backend: String,
    /// Number of entries.
    pub entries: usize,
}
