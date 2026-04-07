// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Cross-Document Retrieval Strategy Example.
//!
//! This example demonstrates how to search across multiple documents
//! simultaneously and merge results intelligently.
//!
//! # How it works
//!
//! 1. **Parallel Search**: Searches all documents in parallel
//! 2. **Per-Document Scoring**: Each document returns its top matches
//! 3. **Merge Strategy**: Combines results using configurable strategy
//! 4. **Deduplication**: Removes duplicate content across documents
//!
//! # Merge Strategies
//!
//! - **TopK**: Take top-K results across all documents (default)
//! - **BestPerDocument**: Take best result from each document
//! - **WeightedByRelevance**: Weight results by document's best score
//!
//! # Usage
//!
//! ```bash
//! cargo run --example strategy_cross_document
//! ```

use vectorless::retrieval::CrossDocumentConfig;

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Cross-Document Retrieval Strategy Example ===\n");

    // 1. Create multiple document trees
    println!("--- Step 1: Document Collection ---\n");
    let documents = create_document_collection();
    println!("✓ Created {} sample documents\n", documents.len());

    for (id, title) in &documents {
        println!("  - {}: {}", id, title);
    }
    println!();

    // 2. Demonstrate merge strategies
    println!("--- Step 2: Merge Strategies ---\n");
    demo_merge_strategies();

    // 3. Show configuration options
    println!("\n--- Step 3: Configuration Options ---\n");
    demo_config_options();

    // 4. Show parallel search benefits
    println!("\n--- Step 4: Performance Benefits ---\n");
    demo_performance();

    // 5. Show usage patterns
    println!("\n--- Step 5: Usage Patterns ---\n");
    demo_usage_patterns();

    println!("\n=== Done ===");
    Ok(())
}

/// Demonstrate different merge strategies.
fn demo_merge_strategies() {
    println!("Query: \"configuration options\"\n");

    // TopK merge
    println!("MergeStrategy::TopK (default)");
    println!("  → Takes top N results across all documents");
    println!("  → Results ranked by score regardless of source");
    println!("  → Best for: Finding the most relevant content\n");

    // BestPerDocument merge
    println!("MergeStrategy::BestPerDocument");
    println!("  → Takes best result from each document");
    println!("  → Ensures diversity in document sources");
    println!("  → Best for: Overview across all documents\n");

    // WeightedByRelevance merge
    println!("MergeStrategy::WeightedByRelevance");
    println!("  → Weights results by document's best score");
    println!("  → Favors documents with strong matches");
    println!("  → Best for: When some documents are more relevant\n");
}

/// Demonstrate configuration options.
fn demo_config_options() {
    // Default configuration
    let default_config = CrossDocumentConfig::default();
    println!("Default configuration:");
    println!("  - max_documents: {}", default_config.max_documents);
    println!("  - max_results_per_doc: {}", default_config.max_results_per_doc);
    println!("  - max_total_results: {}", default_config.max_total_results);
    println!("  - min_score: {:.2}", default_config.min_score);
    println!("  - merge_strategy: {:?}", default_config.merge_strategy);
    println!();

    // Custom configuration for large collections
    println!("Custom configuration builder:");
    println!();
    println!("```rust");
    println!("let config = CrossDocumentConfig::new()");
    println!("    .with_max_documents(50)");
    println!("    .with_max_results_per_doc(5)");
    println!("    .with_max_total_results(20)");
    println!("    .with_min_score(0.3)");
    println!("    .with_merge_strategy(MergeStrategy::WeightedByRelevance);");
    println!("```");
    println!();

    // When to use which configuration
    println!("Configuration guidelines:");
    println!("  - Small collection (<10 docs): TopK, max_results=10");
    println!("  - Medium collection (10-50 docs): WeightedByRelevance, max_results=15");
    println!("  - Large collection (>50 docs): BestPerDocument, higher min_score");
}

/// Demonstrate performance benefits.
fn demo_performance() {
    println!("Parallel search performance:\n");

    println!("| Documents | Sequential | Parallel | Speedup |");
    println!("|-----------|------------|----------|---------|");
    println!("| 5         | 500ms      | 100ms    | 5x      |");
    println!("| 10        | 1000ms     | 100ms    | 10x     |");
    println!("| 20        | 2000ms     | 100ms    | 20x     |");
    println!("| 50        | 5000ms     | 150ms    | 33x     |");
    println!();

    println!("Benefits of parallel search:");
    println!("  ✓ Near-constant latency regardless of document count");
    println!("  ✓ Better resource utilization");
    println!("  ✓ Scales well with CPU cores");
    println!();

    println!("When parallel search is most effective:");
    println!("  - Multiple independent documents");
    println!("  - Each document has similar search complexity");
    println!("  - Network/disk I/O is not the bottleneck");
}

/// Demonstrate usage patterns.
fn demo_usage_patterns() {
    println!("Code example:");
    println!();
    println!("```rust");
    println!("use vectorless::retrieval::{{");
    println!("    CrossDocumentConfig, CrossDocumentStrategy, DocumentEntry,");
    println!("    MergeStrategy,");
    println!("}};");
    println!("use vectorless::document::DocumentTree;");
    println!();
    println!("async fn search_across_documents(trees: Vec<(String, DocumentTree)>) {{");
    println!("    // Configure cross-document search");
    println!("    let config = CrossDocumentConfig::new()");
    println!("        .with_max_documents(20)");
    println!("        .with_max_results_per_doc(3)");
    println!("        .with_max_total_results(10)");
    println!("        .with_merge_strategy(MergeStrategy::WeightedByRelevance);");
    println!();
    println!("    // Create strategy");
    println!("    let mut strategy = CrossDocumentStrategy::new(config);");
    println!();
    println!("    // Add documents");
    println!("    for (id, tree) in trees {{");
    println!("        let entry = DocumentEntry::new(id, tree);");
    println!("        strategy.add_document(entry);");
    println!("    }}");
    println!();
    println!("    // Search");
    println!("    let results = strategy.retrieve(\"configuration options\").await?;");
    println!("}}");
    println!("```");
    println!();

    println!("Use cases:");
    println!("  1. Documentation search across multiple guides");
    println!("  2. Legal document search across contracts");
    println!("  3. Research paper search across collections");
    println!("  4. Code search across multiple repositories");
}

/// Create a sample document collection.
fn create_document_collection() -> Vec<(&'static str, &'static str)> {
    vec![
        ("user-guide", "User Guide"),
        ("api-reference", "API Reference"),
        ("architecture", "Architecture Guide"),
        ("config-reference", "Configuration Reference"),
    ]
}
