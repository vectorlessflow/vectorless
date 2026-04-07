// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! In-memory storage backend (for testing).

use std::collections::HashMap;
use std::sync::RwLock;

use super::StorageBackend;
use crate::error::Result;

/// In-memory storage backend.
///
/// Stores all data in a `HashMap`. Useful for testing and scenarios
/// where persistence is not required.
///
/// # Thread Safety
///
/// Uses `RwLock` for thread-safe access to the internal map.
#[derive(Debug, Default)]
pub struct MemoryBackend {
    /// Internal storage.
    data: RwLock<HashMap<String, Vec<u8>>>,
}

impl MemoryBackend {
    /// Create a new in-memory backend.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new in-memory backend with pre-seeded data.
    pub fn with_data(data: HashMap<String, Vec<u8>>) -> Self {
        Self {
            data: RwLock::new(data),
        }
    }
}

impl StorageBackend for MemoryBackend {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let data = self
            .data
            .read()
            .map_err(|_| crate::Error::Cache("Memory backend lock poisoned".to_string()))?;
        Ok(data.get(key).cloned())
    }

    fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        let mut data = self
            .data
            .write()
            .map_err(|_| crate::Error::Cache("Memory backend lock poisoned".to_string()))?;
        data.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<bool> {
        let mut data = self
            .data
            .write()
            .map_err(|_| crate::Error::Cache("Memory backend lock poisoned".to_string()))?;
        Ok(data.remove(key).is_some())
    }

    fn exists(&self, key: &str) -> Result<bool> {
        let data = self
            .data
            .read()
            .map_err(|_| crate::Error::Cache("Memory backend lock poisoned".to_string()))?;
        Ok(data.contains_key(key))
    }

    fn keys(&self) -> Result<Vec<String>> {
        let data = self
            .data
            .read()
            .map_err(|_| crate::Error::Cache("Memory backend lock poisoned".to_string()))?;
        Ok(data.keys().cloned().collect())
    }

    fn len(&self) -> Result<usize> {
        let data = self
            .data
            .read()
            .map_err(|_| crate::Error::Cache("Memory backend lock poisoned".to_string()))?;
        Ok(data.len())
    }

    fn clear(&self) -> Result<()> {
        let mut data = self
            .data
            .write()
            .map_err(|_| crate::Error::Cache("Memory backend lock poisoned".to_string()))?;
        data.clear();
        Ok(())
    }

    fn batch_put(&self, items: &[(&str, &[u8])]) -> Result<()> {
        let mut data = self
            .data
            .write()
            .map_err(|_| crate::Error::Cache("Memory backend lock poisoned".to_string()))?;
        for (key, value) in items {
            data.insert(key.to_string(), value.to_vec());
        }
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "memory"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_backend_basic() {
        let backend = MemoryBackend::new();

        // Put and get
        backend.put("key1", b"value1").unwrap();
        let value = backend.get("key1").unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Non-existent key
        let missing = backend.get("missing").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_memory_backend_delete() {
        let backend = MemoryBackend::new();

        backend.put("key1", b"value1").unwrap();
        assert!(backend.exists("key1").unwrap());

        let deleted = backend.delete("key1").unwrap();
        assert!(deleted);
        assert!(!backend.exists("key1").unwrap());

        // Delete non-existent
        let not_deleted = backend.delete("missing").unwrap();
        assert!(!not_deleted);
    }

    #[test]
    fn test_memory_backend_keys() {
        let backend = MemoryBackend::new();

        backend.put("key1", b"v1").unwrap();
        backend.put("key2", b"v2").unwrap();
        backend.put("key3", b"v3").unwrap();

        let keys = backend.keys().unwrap();
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn test_memory_backend_clear() {
        let backend = MemoryBackend::new();

        backend.put("key1", b"v1").unwrap();
        backend.put("key2", b"v2").unwrap();

        backend.clear().unwrap();
        assert!(backend.is_empty().unwrap());
    }

    #[test]
    fn test_memory_backend_with_data() {
        let mut initial = HashMap::new();
        initial.insert("k1".to_string(), b"v1".to_vec());
        initial.insert("k2".to_string(), b"v2".to_vec());

        let backend = MemoryBackend::with_data(initial);
        assert_eq!(backend.len().unwrap(), 2);
    }
}
