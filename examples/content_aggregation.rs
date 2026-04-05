// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Content Aggregation Accuracy Example
//!
//! This example demonstrates the content aggregation module's ability to:
//! 1. Score content relevance
//! 2. Allocate token budget
//! 3. Build structured output
//!
//! # Usage
//!
//! ```bash
//! cargo run --example content_aggregation
//! ```

use vectorless::retrieval::content::{
    ContentAggregator, ContentAggregatorConfig, BudgetAllocator, AllocationStrategy,
    StructureBuilder, OutputFormat, RelevanceScorer, ScoringStrategyConfig,
    ContentChunk, ScoringContext,
};
use vectorless::document::NodeId;
use indextree::Arena;

fn make_node_id() -> NodeId {
    let mut arena = Arena::new();
    let node = vectorless::document::TreeNode {
        title: "Test".to_string(),
        structure: String::new(),
        content: String::new(),
        summary: String::new(),
        depth: 0,
        start_index: 0,
        end_index: 0,
        start_page: None,
        end_page: None,
        node_id: None,
        physical_index: None,
        token_count: None,
    };
    NodeId(arena.new_node(node))
}

fn main() {
    println!("=== Content Aggregation Accuracy Demo ===\n");

    // 1. Demonstrate Relevance Scoring
    println!("1. Relevance Scoring Demo");
    println!("---------------------------");

    let query = "What is the architecture of vectorless?";
    let scorer = RelevanceScorer::new(query, ScoringStrategyConfig::KeywordWithBM25);

    let chunks = vec![
        ContentChunk::new(
            make_node_id(),
            "Architecture Overview".to_string(),
            "Vectorless uses a tree-based architecture for document navigation. The system consists of multiple stages: parsing, indexing, and retrieval.".to_string(),
            0,
        ),
        ContentChunk::new(
            make_node_id(),
            "Installation Guide".to_string(),
            "To install vectorless, add it to your Cargo.toml file. Then run cargo build to compile.".to_string(),
            1,
        ),
        ContentChunk::new(
            make_node_id(),
            "Core Components".to_string(),
            "The architecture includes Pilot for navigation, Judge for sufficiency checking, and multiple search algorithms like beam search and greedy search.".to_string(),
            1,
        ),
    ];

    let ctx = ScoringContext::default();

    println!("Query: \"{}\"", query);
    println!("\nScored chunks:");
    for chunk in &chunks {
        let relevance = scorer.score_chunk(chunk, &ctx);
        println!("  - '{}' (depth {}): score {:.3}",
            chunk.title, chunk.depth, relevance.score);
        println!("    Components: keyword={:.2}, bm25={:.2}, depth_penalty={:.2}, density={:.2}",
            relevance.components.keyword_score,
            relevance.components.bm25_score,
            relevance.components.depth_penalty,
            relevance.components.density_score,
        );
    }

    // 2. Demonstrate Budget Allocation
    println!("\n\n2. Budget Allocation Demo");
    println!("---------------------------");

    let scored: Vec<_> = chunks
        .iter()
        .map(|chunk| scorer.score_chunk(chunk, &ctx))
        .collect();

    let strategies = vec![
        ("Greedy", AllocationStrategy::Greedy),
        ("Hierarchical (20%/level)", AllocationStrategy::Hierarchical { min_per_level: 0.2 }),
    ];

    for (name, strategy) in strategies {
        let allocator = BudgetAllocator::new(200)
            .with_strategy(strategy);

        let result = allocator.allocate(scored.clone(), 2);

        println!("\n{} Strategy:", name);
        println!("  Tokens used: {}/{}", result.tokens_used, 200);
        println!("  Items selected: {}", result.selected.len());
        println!("  Avg score: {:.3}", result.stats.avg_score);

        for content in &result.selected {
            let trunc = if content.is_truncated() { " [truncated]" } else { "" };
            println!("    - '{}' ({} tokens, score {:.2}){}",
                content.title, content.tokens, content.score, trunc);
        }
    }

    // 3. Demonstrate Structure Building
    println!("\n\n3. Structure Building Demo");
    println!("---------------------------");

    let formats = vec![
        ("Markdown", OutputFormat::Markdown),
        ("Flat", OutputFormat::Flat),
    ];

    let allocator = BudgetAllocator::new(500)
        .with_strategy(AllocationStrategy::Greedy);
    let result = allocator.allocate(scored.clone(), 2);

    for (name, format) in formats {
        let builder = StructureBuilder::new(format);
        let tree = vectorless::document::DocumentTree::new("Test", "");
        let structured = builder.build(result.selected.clone(), &tree);

        println!("\n{} Output ({} chars, {} tokens):", name, structured.content.len(), structured.metadata.total_tokens);
        let preview = if structured.content.len() > 300 {
            format!("{}...", &structured.content[..300])
        } else {
            structured.content.clone()
        };
        println!("{}", preview.lines().take(8).collect::<Vec<_>>().join("\n"));
    }

    // 4. Demonstrate Full Aggregation Pipeline
    println!("\n\n4. Full Aggregation Pipeline Demo");
    println!("-----------------------------------");

    let configs = vec![
        ("Default (4000 tokens)", ContentAggregatorConfig::default()),
        ("Conservative (1000 tokens)", ContentAggregatorConfig::new()
            .with_token_budget(1000)
            .with_min_relevance(0.3)),
        ("High Precision (2000 tokens, 0.5 threshold)", ContentAggregatorConfig::new()
            .with_token_budget(2000)
            .with_min_relevance(0.5)),
    ];

    for (name, config) in configs {
        println!("\n{} Config:", name);
        println!("  Token budget: {}", config.token_budget);
        println!("  Min relevance: {:.1}", config.min_relevance_score);

        let aggregator = ContentAggregator::new(config);
        // Note: Full aggregation requires a DocumentTree with actual content
        let _ = aggregator; // Suppress unused warning
    }

    println!("\n=== Demo Complete ===");
}
