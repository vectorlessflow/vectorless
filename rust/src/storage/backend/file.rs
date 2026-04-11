// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! File system storage backend.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use tracing::debug;

use super::StorageBackend;
use crate::Error;
use crate::error::Result;

/// File system storage backend.
///
/// Stores each key-value pair as a separate file in a directory.
/// The key is used as the filename (with `.bin` extension).
///
/// # Structure
///
/// ```text
/// workspace/
/// ├── doc-1.bin           # Document 1
/// ├── doc-2.bin           # Document 2
/// ├── meta.bin            # Metadata index
/// └── .workspace.lock     # Lock file
/// ```
///
/// # Thread Safety
///
/// Uses `RwLock` for thread-safe operations on the directory listing cache.
#[derive(Debug)]
pub struct FileBackend {
    /// Root directory for storage.
    root: PathBuf,
    /// Cached directory listing (refreshed on miss).
    cache: RwLock<Option<Vec<String>>>,
}

impl FileBackend {
    /// Create a new file backend at the given path.
    ///
    /// Creates the directory if it doesn't exist.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let root = path.into();
        fs::create_dir_all(&root).map_err(Error::Io)?;

        Ok(Self {
            root,
            cache: RwLock::new(None),
        })
    }

    /// Open an existing file backend.
    ///
    /// Creates the directory if it doesn't exist.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        Self::new(path)
    }

    /// Get the root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Convert a key to a file path.
    fn key_to_path(&self, key: &str) -> PathBuf {
        // Sanitize key to prevent path traversal
        let sanitized = key.replace("..", "_").replace(['/', '\\', ':'], "_");
        self.root.join(format!("{}.bin", sanitized))
    }

    /// Refresh the directory listing cache.
    fn refresh_cache(&self) -> Result<Vec<String>> {
        let entries: Vec<String> = fs::read_dir(&self.root)
            .map_err(Error::Io)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension()?.to_str()? == "bin" {
                    path.file_stem()?.to_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            *cache = Some(entries.clone());
        }

        Ok(entries)
    }

    /// Get cached keys or refresh cache.
    fn get_keys(&self) -> Result<Vec<String>> {
        // Try to read from cache first
        if let Ok(cache) = self.cache.read() {
            if let Some(ref keys) = *cache {
                return Ok(keys.clone());
            }
        }

        // Refresh cache
        self.refresh_cache()
    }

    /// Invalidate the cache.
    pub fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            *cache = None;
        }
    }
}

impl StorageBackend for FileBackend {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let path = self.key_to_path(key);

        if !path.exists() {
            return Ok(None);
        }

        let data = fs::read(&path).map_err(Error::Io)?;
        debug!("Read {} bytes from {}", data.len(), key);

        Ok(Some(data))
    }

    fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        let path = self.key_to_path(key);

        // Use atomic write (temp file + rename)
        let temp_path = path.with_extension("tmp");

        fs::write(&temp_path, value).map_err(Error::Io)?;
        fs::rename(&temp_path, &path).map_err(Error::Io)?;

        // Invalidate cache
        self.invalidate_cache();

        debug!("Wrote {} bytes to {}", value.len(), key);
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<bool> {
        let path = self.key_to_path(key);

        if !path.exists() {
            return Ok(false);
        }

        fs::remove_file(&path).map_err(Error::Io)?;

        // Invalidate cache
        self.invalidate_cache();

        debug!("Deleted {}", key);
        Ok(true)
    }

    fn exists(&self, key: &str) -> Result<bool> {
        let path = self.key_to_path(key);
        Ok(path.exists())
    }

    fn keys(&self) -> Result<Vec<String>> {
        self.get_keys()
    }

    fn len(&self) -> Result<usize> {
        Ok(self.get_keys()?.len())
    }

    fn clear(&self) -> Result<()> {
        let keys = self.get_keys()?;

        for key in &keys {
            let path = self.key_to_path(key);
            if path.exists() {
                fs::remove_file(&path).map_err(Error::Io)?;
            }
        }

        // Clear cache
        if let Ok(mut cache) = self.cache.write() {
            *cache = None;
        }

        debug!("Cleared {} entries", keys.len());
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "file"
    }

    fn batch_put(&self, items: &[(&str, &[u8])]) -> Result<()> {
        for (key, value) in items {
            self.put(key, value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_file_backend_basic() {
        let temp = TempDir::new().unwrap();
        let backend = FileBackend::new(temp.path()).unwrap();

        // Put and get
        backend.put("key1", b"value1").unwrap();
        let value = backend.get("key1").unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Exists
        assert!(backend.exists("key1").unwrap());
        assert!(!backend.exists("key2").unwrap());

        // Delete
        assert!(backend.delete("key1").unwrap());
        assert!(!backend.exists("key1").unwrap());
        assert!(!backend.delete("key1").unwrap()); // Already deleted
    }

    #[test]
    fn test_file_backend_keys() {
        let temp = TempDir::new().unwrap();
        let backend = FileBackend::new(temp.path()).unwrap();

        backend.put("key1", b"v1").unwrap();
        backend.put("key2", b"v2").unwrap();
        backend.put("key3", b"v3").unwrap();

        let keys = backend.keys().unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
    }

    #[test]
    fn test_file_backend_clear() {
        let temp = TempDir::new().unwrap();
        let backend = FileBackend::new(temp.path()).unwrap();

        backend.put("key1", b"v1").unwrap();
        backend.put("key2", b"v2").unwrap();

        backend.clear().unwrap();

        assert!(backend.is_empty().unwrap());
    }

    #[test]
    fn test_file_backend_batch() {
        let temp = TempDir::new().unwrap();
        let backend = FileBackend::new(temp.path()).unwrap();

        let items: Vec<(&str, &[u8])> = vec![
            ("k1", b"v1".as_slice()),
            ("k2", b"v2".as_slice()),
            ("k3", b"v3".as_slice()),
        ];

        backend.batch_put(&items).unwrap();

        let results = backend.batch_get(&["k1", "k2", "k3", "k4"]).unwrap();
        assert_eq!(results.len(), 4);
        assert!(results[0].is_some());
        assert!(results[3].is_none());
    }

    #[test]
    fn test_file_backend_key_sanitization() {
        let temp = TempDir::new().unwrap();
        let backend = FileBackend::new(temp.path()).unwrap();

        // Keys with special characters should be sanitized
        backend.put("../etc/passwd", b"malicious").unwrap();
        backend.put("path/to/file", b"nested").unwrap();

        // Both should be stored safely
        assert!(backend.exists("../etc/passwd").unwrap());
        assert!(backend.exists("path/to/file").unwrap());
    }
}
