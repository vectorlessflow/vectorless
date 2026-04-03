// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index example - demonstrates the document indexing.
//!
//! This example shows how to:
//! 1. Create an index pipeline executor
//! 2. Configure pipeline options
//! 3. Execute the pipeline on a document
//! 4. Inspect the generated document tree
//!
//! # Usage
//!
//! ```bash
//! cargo run --example index
//! ```

use vectorless::index::{PipelineExecutor, PipelineOptions, IndexInput};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Index Pipeline Example ===\n");

    // 1. Create pipeline executor
    let mut executor = PipelineExecutor::new();
    println!("✓ Created pipeline executor\n");

    // 2. Configure pipeline options
    let options = PipelineOptions::default();
    println!("Pipeline options:");
    println!("  - Generate IDs: {}", options.generate_ids);
    println!("  - Generate description: {}", options.generate_description);
    println!();

    // 3. Create input from a file
    let input = IndexInput::file("./README.md");
    println!("Input: ./README.md\n");

    // 4. Execute the pipeline
    println!("Executing pipeline stages:");
    println!("  [1/5] Parse     - Parse document into tree structure");
    println!("  [2/5] Build     - Build document tree with metadata");
    println!("  [3/5] Enhance   - Add ToC and section detection");
    println!("  [4/5] Enrich    - Generate summaries for nodes");
    println!("  [5/5] Optimize  - Optimize tree structure");
    println!();

    let result = executor.execute(input, options).await?;
    println!("✓ Pipeline completed\n");

    // 5. Inspect the result
    println!("Results:");
    println!("  - Document name: {}", result.name);

    if let Some(ref description) = result.description {
        let preview: String = description.chars().take(100).collect();
        println!("  - Description: {}...", preview);
    }

    if let Some(ref tree) = result.tree {
        println!("  - Tree nodes: {}", tree.node_count());
        println!();

        // Print tree structure (first 2 levels)
        println!("Document structure:");
        print_tree_structure(&tree, tree.root(), 0, 2);
    }

    if let Some(page_count) = result.page_count {
        println!("\n  - Pages: {}", page_count);
    }

    println!("\n=== Done ===");
    Ok(())
}

/// Print tree structure up to a maximum depth.
fn print_tree_structure(
    tree: &vectorless::domain::DocumentTree,
    node_id: vectorless::domain::NodeId,
    current_depth: usize,
    max_depth: usize,
) {
    if current_depth > max_depth {
        return;
    }

    let indent = "  ".repeat(current_depth);

    if let Some(node) = tree.get(node_id) {
        let children = tree.children(node_id);
        let marker = if children.is_empty() { "└─" } else { "├─" };
        println!("{}{} {} (depth: {})", indent, marker, node.title, node.depth);

        for child_id in children {
            print_tree_structure(tree, child_id, current_depth + 1, max_depth);
        }
    }
}
