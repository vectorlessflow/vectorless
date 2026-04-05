// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Feedback Learning example.
//!
//! This example demonstrates how to use the feedback learning system
//! to improve Pilot decision quality over time.
//!
//! # What you'll learn:
//! - How to create a FeedbackStore for collecting feedback
//! - How to integrate PilotLearner with LlmPilot
//! - How to record user feedback for decisions
//! - How the learner automatically adjusts decisions
//!
//! # Key concepts:
//!
//! ## Feedback Flow
//! ```text
//! Retrieval → Decision → User Feedback → FeedbackStore
//!                ↑                              ↓
//!                └──────── PilotLearner ────────┘
//!                     (adjusts confidence)
//! ```
//!
//! ## Learning Effect
//! - High accuracy scenarios → Pilot confidence boosted
//! - Low accuracy scenarios → Algorithm trusted more
//! - Very low accuracy → Intervention skipped entirely

use std::sync::Arc;
use vectorless::document::DocumentTree;
use vectorless::llm::LlmClient;
use vectorless::retrieval::pilot::feedback::{
    FeedbackRecord, FeedbackStore, FeedbackStoreConfig, LearnerConfig, PilotLearner,
    DecisionId, SubQueryComplexity, SubQueryType,
};
use vectorless::retrieval::pilot::{InterventionPoint, LlmPilot, PilotConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Feedback Learning Example ===\n");

    // 1. Create FeedbackStore with in-memory storage
    let store = Arc::new(FeedbackStore::in_memory());
    println!("✓ Created FeedbackStore (in-memory)");

    // 2. Create Learner with custom configuration
    let learner_config = LearnerConfig {
        min_samples: 5,               // Need 5 samples before adjusting
        high_accuracy_threshold: 0.8, // 80%+ accuracy = boost confidence
        low_accuracy_threshold: 0.5,  // 50%- accuracy = reduce confidence
        max_confidence_delta: 0.2,    // Max adjustment ±0.2
    };
    let learner = Arc::new(PilotLearner::with_config(store.clone(), learner_config));
    println!("✓ Created PilotLearner with custom config");

    // 3. Create LlmPilot with feedback learning
    let client = LlmClient::for_model("gpt-4o-mini");
    let pilot = LlmPilot::new(client, PilotConfig::default()).with_learner(learner.clone());
    println!("✓ Created LlmPilot with feedback learner");

    // 4. Simulate some retrieval operations with feedback
    println!("\n=== Simulating Retrieval with Feedback ===\n");

    // Simulate 10 retrieval operations
    for i in 0..10 {
        let decision_id = DecisionId(i);
        let was_correct = i % 3 != 0; // 66% accuracy
        let confidence = 0.7 + (i as f64 * 0.02);

        // Create feedback record
        let record = FeedbackRecord::new(
            decision_id,
            was_correct,
            confidence,
            InterventionPoint::Fork,
            12345, // query_hash
            67890, // path_hash
        );

        // Record feedback
        pilot.record_feedback(record);

        println!(
            "Decision {}: {} (confidence: {:.2})",
            i,
            if was_correct { "✓ Correct" } else { "✗ Incorrect" },
            confidence
        );
    }

    // 5. View learning statistics
    println!("\n=== Learning Statistics ===\n");

    let stats = store.intervention_stats();
    println!("Fork Point Statistics:");
    println!("  Total decisions: {}", stats.fork.total);
    println!("  Correct: {}", stats.fork.correct);
    println!("  Accuracy: {:.1}%", stats.fork.accuracy() * 100.0);
    println!(
        "  Avg confidence (correct): {:.2}",
        stats.fork.avg_confidence_correct
    );
    println!(
        "  Avg confidence (incorrect): {:.2}",
        stats.fork.avg_confidence_incorrect
    );

    let overall = store.overall_accuracy();
    println!("\nOverall accuracy: {:.1}%", overall * 100.0);
    println!("Total records: {}", store.total_records());

    // 6. Check if learner has enough data
    println!("\n=== Learner Status ===\n");
    if learner.has_sufficient_data() {
        println!("✓ Learner has sufficient data for adjustments");

        // Get adjustment for similar context
        let adjustment = learner.get_adjustment(InterventionPoint::Fork, 12345, 67890);
        println!("\nAdjustment for similar context:");
        println!("  Confidence delta: {:.3}", adjustment.confidence_delta);
        println!("  Algorithm weight: {:.2}", adjustment.algorithm_weight);
        println!(
            "  Skip intervention: {}",
            adjustment.skip_intervention
        );
    } else {
        println!("✗ Learner needs more data before adjusting");
    }

    // 7. Demonstrate persistence (optional)
    println!("\n=== Persistence (Optional) ===\n");

    let persistent_config = FeedbackStoreConfig::with_persistence("/tmp/feedback.json");
    let persistent_store = FeedbackStore::new(persistent_config);

    // In a real app, you would:
    // - Load existing feedback at startup: persistent_store.load()?
    // - Save periodically: persistent_store.persist()?

    println!("To enable persistence, create FeedbackStore with:");
    println!("  FeedbackStoreConfig::with_persistence(\"/path/to/feedback.json\")");

    println!("\n=== Example Complete ===");
    Ok(())
}
