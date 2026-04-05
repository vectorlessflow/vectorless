// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Basic workspace usage example.
//!
//! This example demonstrates the core storage API:
//! - Creating a workspace
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
use vectorless::storage::{DocumentMeta, PersistedDocument, Workspace};

fn main() -> vectorless::Result<()> {
    println!("=== Storage Workspace Example ===\n");

    // Create a temporary workspace
    let workspace_path = "./example_workspace";

    // 1. Create a workspace with custom cache size
    println!("1. Creating workspace at '{}'...", workspace_path);
    let mut workspace = Workspace::with_cache_size(workspace_path, 100)?;
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
    workspace.add(&doc)?;
    println!("   ✓ Document saved\n");

    // 4. List all documents
    println!("4. Listing documents:");
    for id in workspace.list_documents() {
        if let Some(meta) = workspace.get_meta(id) {
            println!("   - {} ({})", meta.doc_name, meta.id);
            if let Some(ref desc) = meta.doc_description {
                println!("     Description: {}", desc);
            }
        }
    }
    println!();

    // 5. Load document (uses LRU cache)
    println!("5. Loading document...");
    let loaded = workspace.load("doc-001")?.expect("Document should exist");
    println!("   ✓ Loaded: {}", loaded.meta.name);
    let root = loaded.tree.root();
    if let Some(node) = loaded.tree.get(root) {
        println!("   Root node title: {}", node.title);
    }
    println!();

    // 6. Cache statistics
    println!("6. Cache statistics:");
    let stats = workspace.cache_stats();
    println!("   - Hits: {}", stats.hits);
    println!("   - Misses: {}", stats.misses);
    println!("   - Evictions: {}", stats.evictions);
    println!(
        "   - Utilization: {:.1}%",
        workspace.cache_utilization() * 100.0
    );
    println!();

    // 7. Load again (should hit cache)
    println!("7. Loading document again (should hit cache)...");
    let _ = workspace.load("doc-001")?;
    let stats = workspace.cache_stats();
    println!("   ✓ Cache hits: {}", stats.hits);
    println!();

    // 8. Remove document
    println!("8. Removing document...");
    let removed = workspace.remove("doc-001")?;
    println!("   ✓ Removed: {}", removed);
    println!("   Workspace is empty: {}", workspace.is_empty());
    println!();

    // Cleanup
    println!("Cleaning up...");
    std::fs::remove_dir_all(workspace_path).ok();
    println!("   ✓ Done!");

    Ok(())
}
