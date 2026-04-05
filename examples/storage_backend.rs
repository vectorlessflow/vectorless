// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Custom storage backend example.
//!
//! This example shows how to implement a custom StorageBackend.
//! Useful for integrating with databases, cloud storage, etc.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example storage_backend
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use vectorless::Result;
use vectorless::document::DocumentTree;
use vectorless::storage::{Workspace, DocumentMeta, PersistedDocument, StorageBackend};

/// A simple in-memory backend with logging.
///
/// This demonstrates how to implement StorageBackend trait.
/// In production, you might implement S3, PostgreSQL, Redis, etc.
#[derive(Debug)]
struct LoggingMemoryBackend {
    name: &'static str,
    data: RwLock<HashMap<String, Vec<u8>>>,
}

impl LoggingMemoryBackend {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl StorageBackend for LoggingMemoryBackend {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let data = self.data.read().unwrap();
        let result = data.get(key).cloned();
        println!(
            "   [{}] GET '{}' -> {}",
            self.name,
            key,
            if result.is_some() {
                "found"
            } else {
                "not found"
            }
        );
        Ok(result)
    }

    fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.insert(key.to_string(), value.to_vec());
        println!("   [{}] PUT '{}' ({} bytes)", self.name, key, value.len());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<bool> {
        let mut data = self.data.write().unwrap();
        let existed = data.remove(key).is_some();
        println!("   [{}] DELETE '{}' -> {}", self.name, key, existed);
        Ok(existed)
    }

    fn exists(&self, key: &str) -> Result<bool> {
        let data = self.data.read().unwrap();
        Ok(data.contains_key(key))
    }

    fn keys(&self) -> Result<Vec<String>> {
        let data = self.data.read().unwrap();
        Ok(data.keys().cloned().collect())
    }

    fn len(&self) -> Result<usize> {
        let data = self.data.read().unwrap();
        Ok(data.len())
    }

    fn clear(&self) -> Result<()> {
        let mut data = self.data.write().unwrap();
        data.clear();
        println!("   [{}] CLEAR", self.name);
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        self.name
    }
}

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Custom Storage Backend Example ===\n");

    // 1. Create custom backend
    println!("1. Creating custom backend...");
    let backend = Arc::new(LoggingMemoryBackend::new("MyCustomBackend"));
    println!("   ✓ Backend: {}\n", backend.backend_name());

    // 2. Create workspace with custom backend
    println!("2. Creating workspace with custom backend...");
    let workspace = Workspace::with_backend(backend).await?;
    println!("   ✓ Workspace created\n");

    // 3. Add a document (watch the logging)
    println!("3. Adding document (observe backend calls):");
    let meta = DocumentMeta::new("custom-doc", "Custom Backend Test", "md");
    let tree = DocumentTree::new("Root", "Testing custom backend!");
    let doc = PersistedDocument::new(meta, tree);
    workspace.add(&doc).await?;
    println!();

    // 4. Load the document
    println!("4. Loading document:");
    let loaded = workspace.load_and_cache("custom-doc").await?.unwrap();
    println!("   ✓ Loaded: {}\n", loaded.meta.name);

    // 5. Show workspace stats
    println!("5. Workspace stats:");
    println!("   - Documents: {}", workspace.len().await);
    println!("   - Cache size: {}", workspace.cache_len().await);
    println!();

    println!("✓ Custom backend example complete!");
    println!("\nTip: Implement StorageBackend to integrate with:");
    println!("  - S3 / GCS / Azure Blob");
    println!("  - PostgreSQL / MySQL");
    println!("  - Redis / Memcached");
    println!("  - Any custom storage system");

    Ok(())
}
