// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieve example - demonstrates the retrieval pipeline.
//!
//! This example shows how to:
//! 1. Create a pipeline retriever
//! 2. Configure retrieval options
//! 3. Execute retrieval queries
//! 4. Use the orchestrator for advanced control
//!
//! # Usage
//!
//! ```bash
//! cargo run --example retrieve
//! ```

use std::sync::Arc;
use vectorless::domain::{DocumentTree, NodeId};
use vectorless::retrieval::{
    PipelineRetriever, RetrieveOptions, Retriever, StrategyPreference,
    pipeline::RetrievalOrchestrator,
    stages::{AnalyzeStage, JudgeStage, PlanStage, SearchStage},
};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Retrieval Pipeline Example ===\n");

    // 1. Create a sample document tree
    let tree = create_sample_tree();
    println!(
        "✓ Created sample document tree ({} nodes)\n",
        tree.node_count()
    );

    // 2. Method A: Use PipelineRetriever (simple API)
    println!("--- Method A: PipelineRetriever (Simple API) ---\n");
    demo_pipeline_retriever(&tree).await?;

    // 3. Method B: Use RetrievalOrchestrator directly (advanced API)
    println!("\n--- Method B: RetrievalOrchestrator (Advanced API) ---\n");
    demo_orchestrator(&tree).await?;

    println!("\n=== Done ===");
    Ok(())
}

/// Demonstrate PipelineRetriever (simple API).
async fn demo_pipeline_retriever(tree: &DocumentTree) -> vectorless::Result<()> {
    // Create retriever with configuration
    let retriever = PipelineRetriever::new()
        .with_max_backtracks(5)
        .with_max_iterations(10);

    println!("PipelineRetriever configuration:");
    println!("  - Max backtracks: 5");
    println!("  - Max iterations: 10");
    println!();

    // Configure retrieval options
    let options = RetrieveOptions {
        top_k: 5,
        beam_width: 3,
        max_iterations: 5,
        max_tokens: 4000,
        sufficiency_check: true,
        include_content: true,
        include_summaries: true,
        strategy: StrategyPreference::Auto,
        ..Default::default()
    };

    println!("RetrieveOptions:");
    println!("  - Top K: {}", options.top_k);
    println!("  - Beam width: {}", options.beam_width);
    println!("  - Max tokens: {}", options.max_tokens);
    println!("  - Sufficiency check: {}", options.sufficiency_check);
    println!();

    // Execute query
    let query = "What is the main architecture?";
    println!("Query: \"{}\"\n", query);

    let response = retriever
        .retrieve(tree, query, &options)
        .await
        .map_err(|e| vectorless::Error::Retrieval(e.to_string()))?;

    // Display results
    println!("Results:");
    println!("  - Is sufficient: {}", response.is_sufficient);
    println!("  - Confidence: {:.2}", response.confidence);
    println!("  - Strategy used: {}", response.strategy_used);
    println!("  - Tokens used: {}", response.tokens_used);
    println!("  - Results count: {}", response.results.len());

    if !response.results.is_empty() {
        println!("\n  Top results:");
        for (i, result) in response.results.iter().take(3).enumerate() {
            println!(
                "    {}. {} (score: {:.2})",
                i + 1,
                result.title,
                result.score
            );
        }
    }

    Ok(())
}

/// Demonstrate RetrievalOrchestrator (advanced API).
async fn demo_orchestrator(tree: &DocumentTree) -> vectorless::Result<()> {
    // Build orchestrator with explicit stages
    let mut orchestrator = RetrievalOrchestrator::new()
        .with_max_backtracks(3)
        .with_max_iterations(5)
        .stage(AnalyzeStage::new())
        .stage(PlanStage::new())
        .stage(SearchStage::new())
        .stage(JudgeStage::new());

    println!("Orchestrator stages:");
    if let Ok(names) = orchestrator.stage_names() {
        for (i, name) in names.iter().enumerate() {
            println!("  {}. {}", i + 1, name);
        }
    }
    println!();

    // Get execution groups (shows parallel potential)
    if let Ok(groups) = orchestrator.get_execution_groups() {
        println!("Execution groups: {} groups", groups.len());
        for (i, group) in groups.iter().enumerate() {
            let parallel = if group.parallel {
                " (can parallelize)"
            } else {
                ""
            };
            println!(
                "  Group {}: {} stages{}",
                i,
                group.stage_indices.len(),
                parallel
            );
        }
    }
    println!();

    // Execute query
    let query = "How does the pipeline work?";
    println!("Query: \"{}\"\n", query);

    let options = RetrieveOptions::default();
    let tree_arc = Arc::new(tree.clone());
    let response = orchestrator
        .execute(tree_arc, query, options)
        .await
        .map_err(|e| vectorless::Error::Retrieval(e.to_string()))?;

    println!("Results:");
    println!("  - Is sufficient: {}", response.is_sufficient);
    println!("  - Confidence: {:.2}", response.confidence);
    println!("  - Complexity: {:?}", response.complexity);
    println!("  - Navigation steps: {}", response.trace.len());

    if !response.trace.is_empty() {
        println!("\n  Navigation trace:");
        for (i, step) in response.trace.iter().take(5).enumerate() {
            println!("    {}. {} (score: {:.2})", i + 1, step.title, step.score);
        }
    }

    Ok(())
}

/// Create a sample document tree for demonstration.
fn create_sample_tree() -> DocumentTree {
    let mut tree = DocumentTree::new(
        "Vectorless Documentation",
        "A hierarchical document intelligence engine written in Rust.",
    );

    // Add sections using the correct API
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
        index_section,
        "Enrich Stage",
        "Generates AI summaries for tree nodes using LLM.",
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
    tree.add_child(
        retrieve_section,
        "Judge Stage",
        "Evaluates sufficiency of collected content, can trigger backtracking.",
    );

    let usage = tree.add_child(tree.root(), "Usage", "How to use the vectorless library.");
    tree.add_child(
        usage,
        "Basic Example",
        "Simple usage with default configuration and workspace.",
    );
    tree.add_child(
        usage,
        "Advanced Example",
        "Custom pipeline configuration with LLM and custom stages.",
    );

    tree
}
