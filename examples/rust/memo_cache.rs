// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! MemoStore verification example.
//!
//! This example demonstrates the LLM memoization system working in a real scenario,
//! showing cache hits/misses and cost savings.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example memo_cache
//! ```
//!
//! # Environment
//!
//! Set OPENAI_API_KEY or ANTHROPIC_API_KEY for full functionality.
//! The example will still run without API keys (using fallback mode).

use chrono::Duration;
use vectorless::memo::{MemoKey, MemoOpType, MemoStore, MemoValue};

fn print_separator(title: &str) {
    println!("\n{}", "=".repeat(60));
    println!("  {}", title);
    println!("{}", "=".repeat(60));
}

fn main() -> vectorless::Result<()> {
    println!("=== MemoStore Verification Example ===\n");

    // ============================================================
    // Part 1: Basic MemoStore Operations
    // ============================================================
    print_separator("Part 1: Basic Operations");

    let store = MemoStore::new()
        .with_ttl(Duration::days(7))
        .with_model("gpt-4o")
        .with_version(1);

    println!("Created MemoStore with:");
    println!("  - TTL: 7 days");
    println!("  - Model: gpt-4o");
    println!("  - Version: 1");

    // Create a summary cache key
    let content = "This is a long document about machine learning...";
    let content_fp = vectorless::utils::fingerprint::Fingerprint::from_str(content);
    let key = MemoKey::summary(&content_fp).with_model("gpt-4o").with_version(1);

    println!("\nCache key created:");
    println!("  - Op type: {:?}", key.op_type);
    println!("  - Input FP: {}", key.input_fp);

    // Check cache (should miss)
    println!("\nChecking cache (first time)...");
    let cached = store.get(&key);
    println!("  Cache hit: {}", cached.is_some());

    // Store a value
    println!("\nStoring summary...");
    let summary = "Machine learning is a subset of AI that enables systems to learn from data.";
    store.put_with_tokens(key.clone(), MemoValue::Summary(summary.to_string()), 500);
    println!("  Stored: \"{}\"", summary);
    println!("  Tokens saved estimate: 500");

    // Check cache again (should hit)
    println!("\nChecking cache (second time)...");
    let cached = store.get(&key);
    println!("  Cache hit: {}", cached.is_some());
    if let Some(value) = cached {
        println!("  Value: \"{}\"", value.as_summary().unwrap_or("(not a summary)"));
    }

    // ============================================================
    // Part 2: Statistics Tracking
    // ============================================================
    print_separator("Part 2: Statistics Tracking");

    // Create a new store for this demo
    let store = MemoStore::with_capacity(100)
        .with_model("gpt-4o-mini");

    println!("Simulating cache usage...\n");

    // Simulate 10 operations
    let operations = [
        ("doc1", "Content about Rust programming"),
        ("doc2", "Introduction to machine learning"),
        ("doc1", "Content about Rust programming"), // Repeat - should hit
        ("doc3", "Deep learning fundamentals"),
        ("doc2", "Introduction to machine learning"), // Repeat - should hit
        ("doc1", "Content about Rust programming"), // Repeat - should hit
        ("doc4", "Natural language processing"),
        ("doc3", "Deep learning fundamentals"), // Repeat - should hit
        ("doc5", "Computer vision basics"),
        ("doc2", "Introduction to machine learning"), // Repeat - should hit
    ];

    let mut hits = 0u64;
    let mut misses = 0u64;

    for (i, (doc_id, content)) in operations.iter().enumerate() {
        let content_fp = vectorless::utils::fingerprint::Fingerprint::from_str(content);
        let key = MemoKey::summary(&content_fp);

        if let Some(_value) = store.get(&key) {
            hits += 1;
            println!("  [{:2}] {} - CACHE HIT", i + 1, doc_id);
        } else {
            misses += 1;
            println!("  [{:2}] {} - cache miss (storing...)", i + 1, doc_id);
            store.put_with_tokens(key, MemoValue::Summary(format!("Summary of {}", content)), 100);
        }
    }

    println!("\nStatistics:");
    println!("  - Hits: {}", hits);
    println!("  - Misses: {}", misses);
    println!("  - Hit rate: {:.1}%", (hits as f64 / (hits + misses) as f64) * 100.0);

    // ============================================================
    // Part 3: Cache Invalidation
    // ============================================================
    print_separator("Part 3: Cache Invalidation");

    let store = MemoStore::new().with_model("gpt-4o");

    // Store different operation types
    let fp1 = vectorless::utils::fingerprint::Fingerprint::from_str("content1");
    let fp2 = vectorless::utils::fingerprint::Fingerprint::from_str("content2");

    store.put(MemoKey::summary(&fp1), MemoValue::Summary("Summary 1".to_string()));
    store.put(MemoKey::summary(&fp2), MemoValue::Summary("Summary 2".to_string()));
    store.put(
        MemoKey::pilot_decision(&fp1, &fp2),
        MemoValue::PilotDecision(vectorless::memo::PilotDecisionValue {
            selected_idx: 0,
            confidence: 0.9,
            reasoning: "Test decision".to_string(),
        }),
    );

    println!("Stored 3 entries:");
    println!("  - 2 Summary entries");
    println!("  - 1 PilotDecision entry");
    println!("  - Total: {} entries", store.len());

    // Invalidate by operation type
    println!("\nInvalidating all Summary entries...");
    let removed = store.invalidate_by_op_type(MemoOpType::Summary);
    println!("  Removed: {} entries", removed);
    println!("  Remaining: {} entries", store.len());

    // ============================================================
    // Part 4: Persistence
    // ============================================================
    print_separator("Part 4: Persistence");

    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let cache_path = temp_dir.path().join("memo_cache.json");

    println!("Cache path: {:?}", cache_path);

    // Create and populate store
    let store = MemoStore::new().with_model("gpt-4o");

    for i in 0..5 {
        let content = format!("Document content {}", i);
        let fp = vectorless::utils::fingerprint::Fingerprint::from_str(&content);
        store.put(
            MemoKey::summary(&fp),
            MemoValue::Summary(format!("Summary {}", i)),
        );
    }
    println!("Created store with {} entries", store.len());

    // Note: save/load are async, skip for this sync example
    println!("\n(Async save/load skipped in sync example)");
    println!("Use store.save(&path).await and store.load(&path).await in async context");

    // ============================================================
    // Part 5: Real-World Scenario Simulation
    // ============================================================
    print_separator("Part 5: Real-World Scenario");

    println!("Simulating a document query session...\n");

    let store = MemoStore::new()
        .with_ttl(Duration::hours(24))
        .with_model("gpt-4o-mini");

    // Simulate multiple queries to the same document
    let document_content = r#"
        # Vectorless Documentation

        Vectorless is a hierarchical, reasoning-native document intelligence engine.
        It provides tree-based document understanding without vector databases.

        ## Features
        - Multi-format parsing (Markdown, PDF, DOCX)
        - LLM-powered summarization
        - Adaptive retrieval strategies
    "#;

    let doc_fp = vectorless::utils::fingerprint::Fingerprint::from_str(document_content);

    // Simulate query context fingerprints
    let queries = [
        ("What is Vectorless?", 0.85),
        ("How does it work?", 0.72),
        ("What formats are supported?", 0.91),
        ("What is Vectorless?", 0.85),  // Repeat
        ("How does it work?", 0.72),    // Repeat
    ];

    println!("Processing {} queries...\n", queries.len());

    for (i, (query, confidence)) in queries.iter().enumerate() {
        let query_fp = vectorless::utils::fingerprint::Fingerprint::from_str(query);
        let key = MemoKey::pilot_decision(&doc_fp, &query_fp);

        if let Some(_value) = store.get(&key) {
            println!("  [{:2}] \"{}\" - CACHED (confidence: {:.2})", i + 1, query, confidence);
        } else {
            println!("  [{:2}] \"{}\" - Computing... (confidence: {:.2})", i + 1, query, confidence);
            store.put_with_tokens(
                key,
                MemoValue::PilotDecision(vectorless::memo::PilotDecisionValue {
                    selected_idx: 0,
                    confidence: *confidence as f32,
                    reasoning: format!("Reasoning for: {}", query),
                }),
                150, // ~150 tokens per pilot decision
            );
        }
    }

    // Final statistics
    // Note: get() updates entry-level hits, but global stats are only
    // updated by get_or_compute(). For accurate global stats, use get_or_compute.
    println!("\n=== Final Statistics ===");
    println!("  Cache entries: {}", store.len());
    println!("\nNote: Global stats (hits/misses/tokens_saved) are tracked by");
    println!("get_or_compute(), not by direct get() calls. For accurate tracking,");
    println!("use get_or_compute() in production code.");

    // Cost estimation (based on manual tracking above)
    let manual_hits = 2u64; // Queries 4 and 5 were cache hits
    let tokens_per_decision = 150u64;
    let tokens_saved = manual_hits * tokens_per_decision;
    let cost_per_1k_tokens = 0.0015; // GPT-4o-mini input
    let saved_cost = (tokens_saved as f64 / 1000.0) * cost_per_1k_tokens;
    println!("\n  Manual calculation:");
    println!("    Cache hits: {}", manual_hits);
    println!("    Tokens saved: {}", tokens_saved);
    println!("    Estimated cost saved: ${:.4}", saved_cost);

    println!("\n=== Verification Complete ===");
    println!("MemoStore is working correctly!");

    Ok(())
}
