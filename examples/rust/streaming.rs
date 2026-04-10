// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Streaming retrieval example.
//!
//! This example demonstrates how to use streaming retrieval
//! to get results incrementally as they are found.
//!
//! # What you'll learn:
//! - How to use `retrieve_streaming()` for progressive results
//! - How to handle RetrieveEvent types
//! - How to display results as they arrive
//!
//! # RetrieveEvent types:
//! - `Started`: Query began, shows planned strategy
//! - `StageCompleted`: A pipeline stage finished
//! - `Backtracking`: Search is backtracking for more data
//! - `Completed`: Query finished with final results
//! - `Error`: An error occurred
//!
//! # Usage
//!
//! ```bash
//! cargo run --example streaming
//! ```

use vectorless::document::DocumentTree;
use vectorless::retrieval::{
    PipelineRetriever, RetrieveEvent, RetrieveOptions, StrategyPreference,
};

#[tokio::main]
async fn main() {
    println!("=== Streaming Retrieval Example ===\n");

    // 1. Create a sample document tree
    let tree = create_sample_tree();
    println!("Created sample document tree ({} nodes)\n", tree.node_count());

    // 2. Create a pipeline retriever
    let retriever = PipelineRetriever::new()
        .with_max_backtracks(3)
        .with_max_iterations(5);

    // 3. Configure options (streaming is just a usage pattern, not a flag)
    let options = RetrieveOptions {
        top_k: 5,
        beam_width: 3,
        max_iterations: 5,
        max_tokens: 4000,
        strategy: StrategyPreference::Auto,
        ..Default::default()
    };

    // 4. Execute streaming query
    let query = "What is the architecture?";
    println!("Query: \"{}\"\n", query);
    println!("--- Streaming Events ---\n");

    let (_handle, mut rx) = retriever.retrieve_streaming(&tree, query, &options);

    // 5. Process events as they arrive
    while let Some(event) = rx.recv().await {
        match event {
            RetrieveEvent::Started { query, strategy } => {
                println!("[Started] query=\"{query}\", strategy={strategy}");
            }
            RetrieveEvent::StageCompleted { stage, elapsed_ms } => {
                println!("[StageCompleted] {stage} ({elapsed_ms}ms)");
            }
            RetrieveEvent::NodeVisited { node_id, title, score } => {
                println!("[NodeVisited] {title} (id={node_id}, score={score:.2})");
            }
            RetrieveEvent::ContentFound { title, preview, score, .. } => {
                let short_preview = if preview.len() > 60 {
                    format!("{}...", &preview[..60])
                } else {
                    preview
                };
                println!("[ContentFound] {title} (score={score:.2}): {short_preview}");
            }
            RetrieveEvent::Backtracking { from, to, reason } => {
                println!("[Backtracking] {from} -> {to}: {reason}");
            }
            RetrieveEvent::SufficiencyCheck { level, tokens } => {
                println!("[SufficiencyCheck] level={level:?}, tokens={tokens}");
            }
            RetrieveEvent::Completed { response } => {
                println!("\n--- Final Results ---");
                println!("Confidence:   {:.2}", response.confidence);
                println!("Sufficient:   {}", response.is_sufficient);
                println!("Strategy:     {}", response.strategy_used);
                println!("Tokens used:  {}", response.tokens_used);
                println!("Results:      {}", response.results.len());

                if !response.results.is_empty() {
                    println!("\nTop results:");
                    for (i, result) in response.results.iter().take(3).enumerate() {
                        println!("  {}. {} (score: {:.2})", i + 1, result.title, result.score);
                    }
                }
                break;
            }
            RetrieveEvent::Error { message } => {
                eprintln!("[Error] {message}");
                break;
            }
        }
    }

    println!("\n=== Done ===");
}

/// Create a sample document tree for demonstration.
fn create_sample_tree() -> DocumentTree {
    let mut tree = DocumentTree::new(
        "Vectorless Documentation",
        "A hierarchical document intelligence engine written in Rust.",
    );

    let _intro = tree.add_child(
        tree.root(),
        "Introduction",
        "Vectorless is a document intelligence engine written in Rust.",
    );

    let arch = tree.add_child(
        tree.root(),
        "Architecture",
        "The system consists of three main components: indexer, retriever, and storage.",
    );

    let index_section = tree.add_child(
        arch,
        "Index Pipeline",
        "The index pipeline processes documents into a tree structure with summaries.",
    );
    let retrieve_section = tree.add_child(
        arch,
        "Retrieval Pipeline",
        "The retrieval pipeline finds relevant content using multi-stage processing.",
    );

    tree.add_child(
        index_section,
        "Parse Stage",
        "Parses documents (Markdown, PDF, DOCX) into structured content.",
    );
    tree.add_child(
        index_section,
        "Build Stage",
        "Builds the document tree with metadata like page numbers and indices.",
    );

    tree.add_child(
        retrieve_section,
        "Analyze Stage",
        "Analyzes query complexity and extracts keywords for matching.",
    );
    tree.add_child(
        retrieve_section,
        "Plan Stage",
        "Selects retrieval strategy (keyword/semantic/LLM) and search algorithm.",
    );
    tree.add_child(
        retrieve_section,
        "Search Stage",
        "Executes tree traversal (greedy/beam/MCTS) to find relevant content.",
    );

    tree
}
