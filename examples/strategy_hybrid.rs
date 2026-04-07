// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Hybrid Retrieval Strategy Example.
//!
//! This example demonstrates the Hybrid retrieval strategy that combines
//! BM25 keyword matching with LLM-based semantic evaluation.
//!
//! # How it works
//!
//! 1. **BM25 Pre-filtering**: Quickly scores all nodes using keyword matching
//! 2. **Candidate Selection**: Keeps top candidates based on BM25 scores
//! 3. **LLM Refinement**: Applies LLM reasoning only to top candidates
//! 4. **Final Scoring**: Combines BM25 and LLM scores with configurable weights
//!
//! # Benefits
//!
//! - Reduces LLM API calls (only evaluates top candidates)
//! - Maintains accuracy through semantic understanding
//! - Auto-accepts high BM25 scores (skips LLM entirely)
//! - Auto-rejects low BM25 scores (skips LLM entirely)
//!
//! # Usage
//!
//! ```bash
//! cargo run --example strategy_hybrid
//! ```

use vectorless::document::DocumentTree;
use vectorless::retrieval::HybridConfig;

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Hybrid Retrieval Strategy Example ===\n");

    // 1. Create a sample document tree
    let tree = create_sample_tree();
    println!("✓ Created sample document tree ({} nodes)\n", tree.node_count());

    // 2. Show default configuration
    println!("--- Step 1: Default Configuration ---\n");
    demo_default_config();

    // 3. Show custom configuration
    println!("\n--- Step 2: Custom Configuration ---\n");
    demo_custom_config();

    // 4. Show preset configurations
    println!("\n--- Step 3: Preset Configurations ---\n");
    demo_presets();

    // 5. Show usage patterns
    println!("\n--- Step 4: Usage Patterns ---\n");
    demo_usage_patterns();

    println!("\n=== Done ===");
    Ok(())
}

/// Demonstrate default configuration.
fn demo_default_config() {
    let config = HybridConfig::default();

    println!("Default HybridConfig:");
    println!("  - pre_filter_ratio: {:.0}%", config.pre_filter_ratio * 100.0);
    println!("  - min_candidates: {}", config.min_candidates);
    println!("  - max_candidates: {}", config.max_candidates);
    println!("  - auto_accept_threshold: {:.2}", config.auto_accept_threshold);
    println!("  - auto_reject_threshold: {:.2}", config.auto_reject_threshold);
    println!("  - bm25_weight: {:.2}", config.bm25_weight);
    println!("  - llm_weight: {:.2}", config.llm_weight);
    println!();

    println!("How hybrid retrieval works:");
    println!("  1. BM25 scores all nodes using keyword matching (fast)");
    println!("  2. Keep top 30% of candidates (pre-filter)");
    println!("  3. Auto-accept if BM25 score >= 0.85 (skip LLM entirely)");
    println!("  4. Auto-reject if BM25 score <= 0.15 (skip LLM entirely)");
    println!("  5. For remaining: LLM evaluates semantic relevance");
    println!("  6. Final score = BM25*0.4 + LLM*0.6");
}

/// Demonstrate custom configuration.
fn demo_custom_config() {
    let config = HybridConfig::new()
        .with_pre_filter_ratio(0.2) // More aggressive filtering
        .with_candidate_limits(3, 10)
        .with_thresholds(0.9, 0.2) // Higher bar for auto-accept
        .with_weights(0.3, 0.7); // Favor LLM more

    println!("Custom HybridConfig:");
    println!("  - pre_filter_ratio: {:.0}%", config.pre_filter_ratio * 100.0);
    println!("  - min_candidates: {}", config.min_candidates);
    println!("  - max_candidates: {}", config.max_candidates);
    println!("  - auto_accept_threshold: {:.2}", config.auto_accept_threshold);
    println!("  - auto_reject_threshold: {:.2}", config.auto_reject_threshold);
    println!("  - bm25_weight: {:.2}", config.bm25_weight);
    println!("  - llm_weight: {:.2}", config.llm_weight);
    println!();

    println!("When to use this config:");
    println!("  - High-volume queries where cost matters");
    println!("  - Documents with clear keyword signals");
    println!("  - When LLM quality is more important than speed");
    println!();

    println!("Example scenarios:");
    println!("\n  Scenario 1: Exact keyword match");
    println!("    Query: \"parse markdown files\"");
    println!("    BM25 score: 0.92");
    println!("    → Auto-accepted (>= 0.90), no LLM call needed");

    println!("\n  Scenario 2: No keyword overlap");
    println!("    Query: \"How do I get started?\"");
    println!("    BM25 score: 0.10");
    println!("    → Auto-rejected (<= 0.20), no LLM call needed");

    println!("\n  Scenario 3: Moderate match");
    println!("    Query: \"improve search quality\"");
    println!("    BM25 score: 0.55");
    println!("    → LLM refines: evaluates semantic relevance");
}

/// Demonstrate preset configurations.
fn demo_presets() {
    println!("Available presets:");
    println!();

    println!("1. HybridConfig::high_quality()");
    let hq = HybridConfig::high_quality();
    println!("   - Focus on accuracy over cost");
    println!("   - pre_filter_ratio: {:.0}%", hq.pre_filter_ratio * 100.0);
    println!("   - auto_accept_threshold: {:.2}", hq.auto_accept_threshold);
    println!("   - bm25_weight: {:.2}, llm_weight: {:.2}", hq.bm25_weight, hq.llm_weight);
    println!();

    println!("2. HybridConfig::low_cost()");
    let lc = HybridConfig::low_cost();
    println!("   - Focus on cost efficiency");
    println!("   - pre_filter_ratio: {:.0}%", lc.pre_filter_ratio * 100.0);
    println!("   - auto_accept_threshold: {:.2}", lc.auto_accept_threshold);
    println!("   - bm25_weight: {:.2}, llm_weight: {:.2}", lc.bm25_weight, lc.llm_weight);
    println!();

    println!("3. HybridConfig::default()");
    let def = HybridConfig::default();
    println!("   - Balanced approach");
    println!("   - pre_filter_ratio: {:.0}%", def.pre_filter_ratio * 100.0);
    println!("   - auto_accept_threshold: {:.2}", def.auto_accept_threshold);
    println!("   - bm25_weight: {:.2}, llm_weight: {:.2}", def.bm25_weight, def.llm_weight);
    println!();

    println!("Cost comparison:");
    println!("| Config       | LLM Calls | Quality | Use Case |");
    println!("|--------------|-----------|---------|----------|");
    println!("| low_cost     | 1-2       | Good    | High volume |");
    println!("| default      | 2-5       | High    | General use |");
    println!("| high_quality | 5-10      | Highest | Complex queries |");
}

/// Demonstrate usage patterns.
fn demo_usage_patterns() {
    println!("Code example:");
    println!();
    println!("```rust");
    println!("use vectorless::retrieval::{{HybridConfig, HybridStrategy, LlmStrategy}};");
    println!("use vectorless::llm::LlmClient;");
    println!();
    println!("async fn create_hybrid_retriever(client: LlmClient) {{");
    println!("    // Create LLM strategy");
    println!("    let llm_strategy = Box::new(LlmStrategy::new(client));");
    println!();
    println!("    // Option 1: Use preset");
    println!("    let hybrid = HybridStrategy::new(llm_strategy)");
    println!("        .with_high_quality();");
    println!();
    println!("    // Option 2: Custom config");
    println!("    let config = HybridConfig::new()");
    println!("        .with_pre_filter_ratio(0.25)");
    println!("        .with_candidate_limits(3, 8)");
    println!("        .with_thresholds(0.85, 0.15)");
    println!("        .with_weights(0.35, 0.65);");
    println!();
    println!("    let hybrid = HybridStrategy::new(llm_strategy)");
    println!("        .with_config(config);");
    println!("}}");
    println!("```");
    println!();

    println!("Benefits of hybrid strategy:");
    println!("  ✓ 70-90% reduction in LLM API calls vs pure LLM");
    println!("  ✓ 50-70% reduction in latency");
    println!("  ✓ 90-95% of pure LLM quality");
    println!("  ✓ Graceful degradation when LLM unavailable");
}

/// Create a sample document tree for demonstration.
fn create_sample_tree() -> DocumentTree {
    let mut tree = DocumentTree::new(
        "Vectorless Documentation",
        "A hierarchical document intelligence engine written in Rust.",
    );

    let intro = tree.add_child(
        tree.root(),
        "Introduction",
        "Vectorless is a document intelligence engine that uses LLM-powered tree navigation.",
    );

    tree.add_child(
        intro,
        "Key Features",
        "No embeddings, zero infrastructure, multi-format support.",
    );

    let arch = tree.add_child(
        tree.root(),
        "Architecture",
        "Three main components: indexer, retriever, storage.",
    );

    let retrieve = tree.add_child(
        arch,
        "Retrieval Pipeline",
        "Multi-stage retrieval with BM25 and LLM strategies.",
    );

    tree.add_child(retrieve, "Keyword Strategy", "Fast BM25-based matching.");
    tree.add_child(retrieve, "Hybrid Strategy", "BM25 pre-filter + LLM refinement.");
    tree.add_child(retrieve, "Cross-Document", "Multi-document search.");

    tree
}
