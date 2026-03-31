// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Cache management for document structures and pages.
//!
//! This module provides functionality for caching frequently accessed
//! document data to improve performance.

use crate::document::StructureNodeDto;
use lru::LruCache;
use serde_json;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

/// Cache for document structures and pages.
pub struct DocumentCache {
    /// Path to the cache directory
    cache_dir: PathBuf,

    /// In-memory cache for structure data (LRU eviction)
    structure_cache: LruCache<String, StructureNodeDto>,
}

impl DocumentCache {
    /// Create a new document cache.
    pub fn new<P: AsRef<Path>>(cache_dir: P, max_memory_items: usize) -> Self {
        // Convert to NonZeroUsize, default to 100 if 0 is provided
        let capacity = NonZeroUsize::new(max_memory_items.max(1))
            .unwrap_or_else(|| NonZeroUsize::new(100).unwrap());

        Self {
            cache_dir: cache_dir.as_ref().to_path_buf(),
            structure_cache: LruCache::new(capacity),
        }
    }

    /// Initialize the cache directory.
    pub fn init(&self) -> Result<(), Error> {
        std::fs::create_dir_all(&self.cache_dir)?;
        Ok(())
    }

    /// Get the cache path for a document.
    fn cache_path(&self, doc_id: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.json", doc_id))
    }

    /// Put structure data in the cache.
    pub fn put_structure(&mut self, doc_id: &str, structure: &StructureNodeDto) -> Result<(), Error> {
        // Save to disk
        let path = self.cache_path(doc_id);
        let json = serde_json::to_string_pretty(structure)
            .map_err(|e| Error::Json(e.to_string()))?;
        std::fs::write(&path, json).map_err(|e| Error::Io(e))?;

        // Update in-memory LRU cache (auto-evicts if full)
        self.structure_cache.put(doc_id.to_string(), structure.clone());

        Ok(())
    }

    /// Get structure data from the cache.
    pub fn get_structure(&mut self, doc_id: &str) -> Result<Option<StructureNodeDto>, Error> {
        // Check in-memory cache first
        if let Some(structure) = self.structure_cache.get(doc_id) {
            return Ok(Some(structure.clone()));
        }

        // Check disk cache
        let path = self.cache_path(doc_id);
        if !path.exists() {
            return Ok(None);
        }

        let json = std::fs::read_to_string(&path).map_err(|e| Error::Io(e))?;
        let structure: StructureNodeDto = serde_json::from_str(&json)
            .map_err(|e| Error::Json(e.to_string()))?;

        // Add to in-memory cache (auto-evicts if full)
        self.structure_cache.put(doc_id.to_string(), structure.clone());

        Ok(Some(structure))
    }

    /// Remove a document from the cache.
    pub fn remove(&mut self, doc_id: &str) -> Result<(), Error> {
        // Remove from memory
        self.structure_cache.pop(doc_id);

        // Remove from disk
        let path = self.cache_path(doc_id);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| Error::Io(e))?;
        }

        Ok(())
    }

    /// Clear all cached data.
    pub fn clear(&mut self) -> Result<(), Error> {
        // Clear memory
        self.structure_cache.clear();

        // Clear disk cache
        if self.cache_dir.exists() {
            for entry in std::fs::read_dir(&self.cache_dir).map_err(|e| Error::Io(e))? {
                let entry = entry.map_err(|e| Error::Io(e))?;
                std::fs::remove_file(entry.path()).map_err(|e| Error::Io(e))?;
            }
        }

        Ok(())
    }

    /// Get the number of items currently in the memory cache.
    pub fn len(&self) -> usize {
        self.structure_cache.len()
    }

    /// Check if the memory cache is empty.
    pub fn is_empty(&self) -> bool {
        self.structure_cache.is_empty()
    }

    /// Check if a key exists in the memory cache (without loading from disk).
    pub fn contains_key(&self, doc_id: &str) -> bool {
        self.structure_cache.contains(doc_id)
    }

    /// Peek at a value in the memory cache without updating LRU (for testing).
    pub fn peek(&self, doc_id: &str) -> Option<&StructureNodeDto> {
        self.structure_cache.peek(doc_id)
    }
}

/// Cache error types.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(String),
}
