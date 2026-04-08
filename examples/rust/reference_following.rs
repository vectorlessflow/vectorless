// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Reference Following Example
//!
//! This example demonstrates the reference following feature which allows
//! the retrieval system to follow in-document references like
//! "see Appendix G" or "refer to Table 5.3".
//!
//! # What you'll learn:
//! - How references are extracted from document content
//! - How references are resolved to actual nodes
//! - How to use ReferenceFollower to expand search results
//!
//! # Key concepts:
//!
//! ## Reference Types
//! - Section: "see Section 2.1", "Section 3.2.1"
//! - Appendix: "see Appendix G", "Appendix A"
//! - Table: "Table 5.3", "refer to Table 1"
//! - Figure: "Figure 2.1", "fig. 3"
//! - Page: "see page 42", "p. 15"
//!
//! ## Resolution Flow
//! ```text
//! Extract References → Resolve to Nodes → Follow → Expand Context
//! ```

use vectorless::document::{
    DocumentTree, ReferenceExtractor,
};
use vectorless::retrieval::{
    expand_with_references, ReferenceConfig, ReferenceFollower,
};

fn main() {
    println!("=== Reference Following Example ===\n");

    // 1. Create a document tree with references
    let tree = create_document_with_references();
    println!("Created document tree with {} nodes\n", tree.node_count());

    // 2. Build retrieval index
    let index = tree.build_retrieval_index();
    println!("Built retrieval index\n");

    // 3. Demonstrate reference extraction
    println!("--- Reference Extraction ---\n");

    let content = "For more details, see Section 2.1 and Appendix G. The data is shown in Table 5.3.";
    let refs = ReferenceExtractor::extract(content);

    println!("Content: \"{}\"\n", content);
    println!("Extracted {} references:", refs.len());
    for r#ref in &refs {
        println!(
            "  - {:?}: '{}' -> target '{}'",
            r#ref.ref_type, r#ref.ref_text, r#ref.target_id
        );
    }
    println!();

    // 4. Demonstrate reference resolution
    println!("--- Reference Resolution ---\n");

    let resolved_refs = ReferenceExtractor::extract_and_resolve(content, &tree, &index);
    println!("Resolved references:");
    for r#ref in &resolved_refs {
        let status = if r#ref.is_resolved() {
            format!("resolved (confidence: {:.2})", r#ref.confidence)
        } else {
            "unresolved".to_string()
        };
        println!(
            "  - {:?}: '{}' -> {}",
            r#ref.ref_type, r#ref.target_id, status
        );
    }
    println!();

    // 5. Demonstrate reference following
    println!("--- Reference Following ---\n");

    let config = ReferenceConfig {
        max_depth: 3,
        max_references: 10,
        follow_pages: true,
        follow_tables_figures: true,
        min_confidence: 0.3,
        ..Default::default()
    };
    let follower = ReferenceFollower::new(config);

    // Get the financial section node (which contains references)
    let financial_node = find_node_by_title(&tree, "Financial Summary");
    if let Some(node_id) = financial_node {
        let followed = follower.follow_from_node(&tree, &index, node_id);

        println!("Following references from 'Financial Summary':");
        for f in &followed {
            let target = if let Some(target_id) = f.target_node {
                let title = tree.get(target_id).map(|n| n.title.as_str()).unwrap_or("?");
                format!("-> '{}' (depth {})", title, f.depth)
            } else {
                "-> (unresolved)".to_string()
            };
            println!(
                "  - {:?} '{}' {}",
                f.reference.ref_type, f.reference.target_id, target
            );
        }
    }
    println!();

    // 6. Demonstrate expansion with references
    println!("--- Expansion with References ---\n");

    let initial_nodes: Vec<_> = tree.children(tree.root());
    println!("Initial nodes: {} (root's children)", initial_nodes.len());

    let expansion = expand_with_references(&tree, &index, &initial_nodes, None);

    println!(
        "After reference expansion: {} total nodes, {} new",
        expansion.all_nodes().len(),
        expansion.expanded_nodes.len()
    );

    if expansion.has_expansion() {
        println!("\nExpanded nodes:");
        for node_id in expansion.new_nodes() {
            if let Some(node) = tree.get(*node_id) {
                println!("  - {}", node.title);
            }
        }
    }
    println!();

    // 7. Show configuration options
    println!("--- Configuration Options ---\n");

    let conservative = ReferenceConfig::conservative();
    let aggressive = ReferenceConfig::aggressive();

    println!("Conservative config:");
    println!("  - Max depth: {}", conservative.max_depth);
    println!("  - Max references: {}", conservative.max_references);

    println!("\nAggressive config:");
    println!("  - Max depth: {}", aggressive.max_depth);
    println!("  - Max references: {}", aggressive.max_references);

    println!("\n=== Done ===");
}

fn create_document_with_references() -> DocumentTree {
    let mut tree = DocumentTree::new("Annual Report", "Company annual financial report.");

    // Main sections
    let _intro = tree.add_child(tree.root(), "Introduction", "Overview of the report.");
    let financial = tree.add_child(
        tree.root(),
        "Financial Summary",
        "Financial overview for 2023. For detailed breakdown, see Section 2.1. Revenue data is in Table 5.3. Additional details in Appendix G.",
    );
    let _appendix = tree.add_child(
        tree.root(),
        "Appendix G",
        "Detailed financial tables and data.",
    );

    // Subsections
    tree.add_child(
        financial,
        "2.1 Revenue",
        "Revenue increased by 15% year over year. See Table 5.3 for breakdown.",
    );

    tree
}

fn find_node_by_title(tree: &DocumentTree, title: &str) -> Option<vectorless::document::NodeId> {
    for node_id in tree.traverse() {
        if let Some(node) = tree.get(node_id) {
            if node.title == title {
                return Some(node_id);
            }
        }
    }
    None
}
