// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Basic workspace usage example.
//!
//! This example demonstrates the core storage API:
//! - Creating an async workspace
//! - Adding documents
//! - Loading documents with LRU cache
//! - Listing and removing documents
//!
//! # Usage
//!
//! ```bash
//! cargo run --example storage_workspace
//! ```

use vectorless::document::DocumentTree;
use vectorless::storage::{Workspace, DocumentMeta, PersistedDocument};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Storage Workspace Example ===\n");

    // Create a temporary workspace
    let workspace_path = "./example_workspace";

    // 1. Create a workspace with custom cache size
    println!("1. Creating workspace at '{}'...", workspace_path);
    let workspace = Workspace::with_cache_size(workspace_path, 100)
        .await
        .map_err(|e| vectorless::Error::Workspace(e.to_string()))?;
    println!("   ✓ Workspace created\n");

    // 2. Create a document
    println!("2. Creating a document...");
    let meta = DocumentMeta::new("doc-001", "Getting Started Guide", "md")
        .with_description("An introduction to the workspace API")
        .with_source_path("./docs/getting-started.md");

    let tree = DocumentTree::new("Introduction", "Welcome to Vectorless storage module!");

    let doc = PersistedDocument::new(meta, tree);
    println!("   ✓ Document created: {}\n", doc.meta.id);

    // 3. Add document to workspace
    println!("3. Adding document to workspace...");
    workspace.add(&doc).await.map_err(|e| vectorless::Error::Workspace(e.to_string()))?;
    println!("   ✓ Document saved\n");

    // 4. List all documents
    println!("4. Listing documents:");
    for id in workspace.list_documents().await {
        if let Some(meta) = workspace.get_meta(&id).await {
            println!("   - {} ({})", meta.doc_name, meta.id);
            if let Some(ref desc) = meta.doc_description {
                println!("     Description: {}", desc);
            }
        }
    }
    println!();

    // 5. Load document (uses LRU cache)
    println!("5. Loading document...");
    let loaded = workspace
        .load_and_cache("doc-001")
        .await
        .map_err(|e| vectorless::Error::Workspace(e.to_string()))?
        .expect("Document should exist");
    println!("   ✓ Loaded: {}", loaded.meta.name);
    let root = loaded.tree.root();
    if let Some(node) = loaded.tree.get(root) {
        println!("   Root node title: {}", node.title);
    }
    println!();

    // 6. Cache statistics
    println!("6. Cache statistics:");
    let stats = workspace.cache_stats().await;
    println!("   - Hits: {}", stats.hits);
    println!("   - Misses: {}", stats.misses);
    println!("   - Evictions: {}", stats.evictions);
    println!(
        "   - Utilization: {:.1}%",
        workspace.cache_utilization().await * 100.0
    );
    println!();

    // 7. Load again (should hit cache)
    println!("7. Loading document again (should hit cache)...");
    let _ = workspace
        .load_and_cache("doc-001")
        .await
        .map_err(|e| vectorless::Error::Workspace(e.to_string()))?;
    let stats = workspace.cache_stats().await;
    println!("   ✓ Cache hits: {}", stats.hits);
    println!();

    // 8. Remove document
    println!("8. Removing document...");
    let removed = workspace
        .remove("doc-001")
        .await
        .map_err(|e| vectorless::Error::Workspace(e.to_string()))?;
    println!("   ✓ Removed: {}", removed);
    println!("   Workspace is empty: {}", workspace.is_empty().await);
    println!();

    // Cleanup
    println!("Cleaning up...");
    std::fs::remove_dir_all(workspace_path).ok();
    println!("   ✓ Done!");

    Ok(())
}
