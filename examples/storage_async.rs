// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Async workspace usage example.
//!
//! This example demonstrates async workspace operations:
//! - Creating an async workspace
//! - Concurrent document access
//! - Async LRU cache
//!
//! # Usage
//!
//! ```bash
//! cargo run --example storage_async
//! ```

use std::sync::Arc;

use vectorless::document::DocumentTree;
use vectorless::storage::{AsyncWorkspace, DocumentMeta, PersistedDocument};

fn create_doc(id: &str, name: &str) -> PersistedDocument {
    let meta = DocumentMeta::new(id, name, "md");
    let content = format!("Content for {}", name);
    let tree = DocumentTree::new("Root", &content);
    PersistedDocument::new(meta, tree)
}

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Async Workspace Example ===\n");

    let workspace_path = "./example_async_workspace";

    // 1. Create async workspace
    println!("1. Creating async workspace...");
    let workspace = AsyncWorkspace::new(workspace_path).await?;
    println!("   ✓ Created\n");

    // 2. Add documents
    println!("2. Adding documents...");
    workspace.add(&create_doc("doc-1", "Document One")).await?;
    workspace.add(&create_doc("doc-2", "Document Two")).await?;
    workspace.add(&create_doc("doc-3", "Document Three")).await?;
    println!("   ✓ Added 3 documents\n");

    // 3. Concurrent access example
    println!("3. Concurrent access from multiple tasks...");
    let ws = Arc::new(workspace);

    let mut handles = vec![];

    // Spawn concurrent read tasks
    for i in 1..=3 {
        let ws_clone = ws.clone();
        let handle = tokio::spawn(async move {
            let id = format!("doc-{}", i);
            let doc = ws_clone.load(&id).await.unwrap().unwrap();
            println!("   [Task {}] Loaded: {}", i, doc.meta.name);
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }
    println!("   ✓ All concurrent loads completed\n");

    // 4. Cache stats
    println!("4. Cache statistics:");
    let stats = ws.cache_stats().await;
    println!("   - Hits: {}", stats.hits);
    println!("   - Misses: {}", stats.misses);
    println!();

    // 5. Clone and share
    println!("5. Workspace can be cloned cheaply (Arc internally)...");
    let ws2 = ws.clone();
    let ws3 = ws.clone();

    let len1 = ws.len().await;
    let len2 = ws2.len().await;
    let len3 = ws3.len().await;

    println!("   ws1.len() = {}, ws2.len() = {}, ws3.len() = {}", len1, len2, len3);
    println!("   ✓ All clones share the same state\n");

    // Cleanup
    println!("Cleaning up...");
    std::fs::remove_dir_all(workspace_path).ok();
    println!("   ✓ Done!");

    Ok(())
}
