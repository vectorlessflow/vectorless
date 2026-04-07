// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Page-Range Retrieval Strategy Example.
//!
//! This example demonstrates how to filter retrieval results by page range,
//! which is particularly useful for PDF documents.
//!
//! # How it works
//!
//! 1. **Page Filtering**: Only considers nodes within specified page range
//! 2. **Boundary Handling**: Configurable handling of nodes spanning boundaries
//! 3. **Context Expansion**: Optionally expands range for surrounding context
//! 4. **Overlap Detection**: Includes nodes that partially overlap with range
//!
//! # Use Cases
//!
//! - "What does chapter 3 say about X?" (pages 45-67)
//! - "Find information in the introduction" (pages 1-10)
//! - "Search the appendix" (pages 200-220)
//!
//! # Usage
//!
//! ```bash
//! cargo run --example strategy_page_range
//! ```

use vectorless::document::DocumentTree;
use vectorless::retrieval::{PageRange, PageRangeConfig};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Page-Range Retrieval Strategy Example ===\n");

    // 1. Create a sample PDF-like document tree with page numbers
    println!("--- Step 1: Document with Page Numbers ---\n");
    let tree = create_pdf_like_tree();
    println!("✓ Created document tree ({} nodes)\n", tree.node_count());

    // 2. Demonstrate page range creation
    println!("--- Step 2: Page Range Options ---\n");
    demo_page_range_options();

    // 3. Show configuration options
    println!("\n--- Step 3: Configuration Options ---\n");
    demo_config_options();

    // 4. Show boundary handling
    println!("\n--- Step 4: Boundary Handling ---\n");
    demo_boundary_handling();

    // 5. Show context expansion
    println!("\n--- Step 5: Context Expansion ---\n");
    demo_context_expansion();

    // 6. Show usage patterns
    println!("\n--- Step 6: Usage Patterns ---\n");
    demo_usage_patterns();

    println!("\n=== Done ===");
    Ok(())
}

/// Demonstrate page range options.
fn demo_page_range_options() {
    println!("PageRange creation methods:\n");

    // Specific range
    let range1 = PageRange::new(10, 20);
    println!("  PageRange::new(10, 20)");
    println!("    → Range: pages 10-20 (inclusive)");
    println!("    → Use case: Search a specific chapter\n");

    // Single page
    let range2 = PageRange::single(15);
    println!("  PageRange::single(15)");
    println!("    → Range: page 15 only");
    println!("    → Use case: Search a specific page\n");

    // From page to end
    let range3 = PageRange::from(30);
    println!("  PageRange::from(30)");
    println!("    → Range: page 30 to end of document");
    println!("    → Use case: Search appendix or references\n");

    // From beginning to page
    let range4 = PageRange::until(10);
    println!("  PageRange::until(10)");
    println!("    → Range: beginning to page 10");
    println!("    → Use case: Search introduction or preface\n");

    // Default (all pages)
    let range5 = PageRange::default();
    println!("  PageRange::default()");
    println!("    → Range: all pages");
    println!("    → Use case: No page restriction\n");

    println!("PageRange methods:");
    println!("  - contains(page): Check if page is in range");
    println!("  - overlaps(start, end): Check if range overlaps");
    println!("  - len(): Get number of pages in range");
    println!("  - is_empty(): Check if range is empty");
}

/// Demonstrate configuration options.
fn demo_config_options() {
    let default_config = PageRangeConfig::default();

    println!("Default PageRangeConfig:");
    println!("  - range: {:?}", default_config.range);
    println!("  - include_boundary_nodes: {}", default_config.include_boundary_nodes);
    println!("  - expand_context_pages: {}", default_config.expand_context_pages);
    println!("  - min_overlap_ratio: {:.2}", default_config.min_overlap_ratio);
    println!();

    println!("Custom configuration:");
    println!();
    println!("```rust");
    println!("let config = PageRangeConfig::new(PageRange::new(10, 30))");
    println!("    .with_boundary_nodes(true)");
    println!("    .with_context_expansion(2)");
    println!("    .with_min_overlap_ratio(0.3);");
    println!("```");
    println!();

    println!("Configuration guidelines:");
    println!("  - Strict range: include_boundary_nodes=false, min_overlap_ratio=1.0");
    println!("  - Include context: expand_context_pages=1-3");
    println!("  - Lenient matching: min_overlap_ratio=0.1");
}

/// Demonstrate boundary handling.
fn demo_boundary_handling() {
    println!("Boundary handling example:\n");

    println!("Scenario: Section spans pages 9-12, query range is 10-15\n");

    println!("  include_boundary_nodes = false (strict)");
    println!("    → Section (9-12) overlaps with range (10-15)");
    println!("    → Included because overlap exists\n");

    println!("  include_boundary_nodes = true (lenient)");
    println!("    → Same result, but also includes partial overlaps");
    println!("    → Useful for comprehensive results\n");

    println!("Overlap calculation:");
    println!("  Section pages: 9-12 (4 pages)");
    println!("  Query range:   10-15 (6 pages)");
    println!("  Overlap:       10-12 (3 pages)");
    println!("  Overlap ratio: 3/4 = 75%\n");

    println!("min_overlap_ratio threshold:");
    println!("  - 0.1 (10%): Include almost any overlap");
    println!("  - 0.5 (50%): Require significant overlap");
    println!("  - 1.0 (100%): Section must be fully within range");
}

/// Demonstrate context expansion.
fn demo_context_expansion() {
    println!("Context expansion example:\n");

    println!("Scenario: Query range is 10-15\n");

    // Without expansion
    println!("  Without expansion (expand_context_pages=0):");
    println!("    → Only pages 10-15 searched");
    println!("    → Might miss related content on pages 9 or 16\n");

    // With expansion
    println!("  With expansion (expand_context_pages=2):");
    println!("    → Effective range: 8-17");
    println!("    → Includes surrounding context for better results\n");

    println!("When to use context expansion:");
    println!("  ✓ When sections span multiple pages");
    println!("  ✓ When relevant content might be just outside range");
    println!("  ✓ For more comprehensive results\n");

    println!("When NOT to use context expansion:");
    println!("  ✗ When you need strict page boundaries");
    println!("  ✗ For chapter-specific queries");
    println!("  ✗ When precision is more important than recall");
}

/// Demonstrate usage patterns.
fn demo_usage_patterns() {
    println!("Code example:");
    println!();
    println!("```rust");
    println!("use vectorless::retrieval::{{PageRange, PageRangeConfig, PageRangeStrategy}};");
    println!("use vectorless::retrieval::RetrievalStrategy;");
    println!();
    println!("async fn search_in_chapter(tree: &DocumentTree) {{");
    println!("    // Search only in chapter 3 (pages 45-67)");
    println!("    let range = PageRange::new(45, 67);");
    println!("    let config = PageRangeConfig::new(range)");
    println!("        .with_boundary_nodes(true)");
    println!("        .with_context_expansion(1);");
    println!();
    println!("    let strategy = PageRangeStrategy::new(config);");
    println!("    ");
    println!("    // Evaluate nodes within page range");
    println!("    let results = strategy.evaluate_nodes(tree, node_ids, context).await;");
    println!("}}");
    println!("```");
    println!();

    println!("Common use cases:");
    println!("  1. Chapter search: PageRange::new(45, 67)");
    println!("  2. Introduction: PageRange::until(10)");
    println!("  3. Appendix: PageRange::from(200)");
    println!("  4. Single page: PageRange::single(42)");
    println!();

    println!("Best practices:");
    println!("  - Know your document's page structure");
    println!("  - Use context_expansion for flowing content");
    println!("  - Use strict boundaries for discrete sections");
    println!("  - Combine with other strategies (hybrid, keyword)");
}

/// Create a sample PDF-like document tree with page numbers.
fn create_pdf_like_tree() -> DocumentTree {
    let mut tree = DocumentTree::new(
        "Sample PDF Document",
        "A sample document simulating PDF structure with page numbers.",
    );

    // Introduction (pages 1-5)
    let intro = tree.add_child(tree.root(), "Introduction", "Overview of the document.");
    tree.set_page_boundaries(intro, 1, 5);
    tree.add_child_with_pages(intro, "Background", "Background information.", 1, 2);
    tree.add_child_with_pages(intro, "Motivation", "Why this document exists.", 3, 4);
    tree.add_child_with_pages(intro, "Scope", "What is covered.", 5, 5);

    // Main Content (pages 6-40)
    let main = tree.add_child(tree.root(), "Main Content", "Primary content sections.");
    tree.set_page_boundaries(main, 6, 40);

    let chapter1 = tree.add_child_with_pages(main, "Chapter 1", "Getting started.", 6, 15);
    tree.add_child_with_pages(chapter1, "Installation", "How to install.", 7, 9);
    tree.add_child_with_pages(chapter1, "Configuration", "Configuration options.", 10, 12);

    let chapter2 = tree.add_child_with_pages(main, "Chapter 2", "Core concepts.", 16, 28);
    tree.add_child_with_pages(chapter2, "Architecture", "System architecture.", 16, 20);
    tree.add_child_with_pages(chapter2, "Data Model", "How data is organized.", 21, 24);

    let chapter3 = tree.add_child_with_pages(main, "Chapter 3", "Advanced usage.", 29, 40);
    tree.add_child_with_pages(chapter3, "Custom Strategies", "Implementing custom strategies.", 29, 33);
    tree.add_child_with_pages(chapter3, "Performance", "Optimizing performance.", 34, 37);

    // Appendix (pages 41-50)
    let appendix = tree.add_child(tree.root(), "Appendix", "Reference materials.");
    tree.set_page_boundaries(appendix, 41, 50);
    tree.add_child_with_pages(appendix, "API Reference", "Complete API documentation.", 41, 45);
    tree.add_child_with_pages(appendix, "Config Reference", "All configuration options.", 46, 48);

    tree
}
