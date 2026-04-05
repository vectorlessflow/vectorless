// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Custom Pilot implementation example.
//!
//! This example demonstrates how to implement a custom Pilot
//! that provides navigation guidance during retrieval.
//!
//! # What you'll learn:
//! - How to implement the Pilot trait
//! - When to intervene (START, FORK, BACKTRACK, EVALUATE)
//! - How to provide ranked candidates
//! - How to integrate custom Pilot with the retrieval pipeline
//!
//! # Key concepts:
//!
//! ## Intervention Points
//! - START: Before search begins - analyze query, set direction
//! - FORK: At branch points - rank candidates, guide path selection
//! - BACKTRACK: When search fails - suggest alternatives
//! - EVALUATE: After content found - check sufficiency
//!
//! ## Score Merging
//! ```text
//! final_score = alpha * algorithm_score + beta * llm_score
//! ```

use async_trait::async_trait;
use std::collections::HashSet;
use vectorless::document::{DocumentTree, NodeId};
use vectorless::retrieval::pilot::{
    InterventionPoint, Pilot, PilotConfig, PilotDecision, RankedCandidate, SearchDirection,
    SearchState,
};

/// A custom Pilot that uses simple keyword matching for guidance.
///
/// This demonstrates the Pilot trait implementation without requiring
/// an actual LLM client.
pub struct KeywordPilot {
    config: PilotConfig,
}

impl KeywordPilot {
    /// Create a new KeywordPilot.
    pub fn new() -> Self {
        Self {
            config: PilotConfig::default(),
        }
    }

    /// Score a node title based on keyword overlap with the query.
    fn score_by_keywords(&self, query: &str, title: &str) -> f32 {
        let query_lower = query.to_lowercase();
        let title_lower = title.to_lowercase();

        let query_words: HashSet<&str> = query_lower
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .collect();

        let title_words: HashSet<&str> = title_lower
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .collect();

        if query_words.is_empty() || title_words.is_empty() {
            return 0.0;
        }

        let overlap = query_words.intersection(&title_words).count();
        overlap as f32 / query_words.len().max(1) as f32
    }
}

impl Default for KeywordPilot {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Pilot for KeywordPilot {
    fn name(&self) -> &str {
        "keyword_pilot"
    }

    fn should_intervene(&self, state: &SearchState<'_>) -> bool {
        // Intervene at fork points with multiple candidates
        if state.candidates.len() > 2 {
            return true;
        }

        // Intervene when best score is low
        if state.best_score < 0.3 {
            return true;
        }

        // Intervene during backtracking
        if state.is_backtracking {
            return true;
        }

        false
    }

    async fn decide(&self, state: &SearchState<'_>) -> PilotDecision {
        // Rank candidates by keyword overlap
        let mut ranked: Vec<RankedCandidate> = state
            .candidates
            .iter()
            .filter_map(|&node_id| {
                state.tree.get(node_id).map(|node| {
                    let score = self.score_by_keywords(state.query, &node.title);
                    RankedCandidate::new(node_id, score)
                })
            })
            .collect();

        ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Determine direction
        let direction = if ranked.is_empty() {
            SearchDirection::backtrack("No candidates available", vec![])
        } else if ranked[0].score > 0.5 {
            SearchDirection::go_deeper(format!("Strong match: {:.2}", ranked[0].score))
        } else if ranked[0].score > 0.2 {
            SearchDirection::go_deeper(format!("Moderate match: {:.2}", ranked[0].score))
        } else {
            SearchDirection::backtrack("No strong matches found", vec![])
        };

        let confidence = ranked.first().map(|c| c.score).unwrap_or(0.0);

        PilotDecision {
            ranked_candidates: ranked,
            direction,
            confidence,
            reasoning: "Keyword-based decision".to_string(),
            intervention_point: InterventionPoint::Fork,
        }
    }

    async fn guide_start(&self, tree: &DocumentTree, query: &str) -> Option<PilotDecision> {
        // Score root's children
        let children = tree.children(tree.root());
        let mut ranked: Vec<RankedCandidate> = children
            .iter()
            .filter_map(|&node_id| {
                tree.get(node_id).map(|node| {
                    let score = self.score_by_keywords(query, &node.title);
                    RankedCandidate::new(node_id, score)
                })
            })
            .collect();

        ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        let confidence = ranked.first().map(|c| c.score).unwrap_or(0.0);

        Some(PilotDecision {
            ranked_candidates: ranked,
            direction: SearchDirection::go_deeper("Starting search"),
            confidence,
            reasoning: "Keyword-based start guidance".to_string(),
            intervention_point: InterventionPoint::Start,
        })
    }

    async fn guide_backtrack(&self, state: &SearchState<'_>) -> Option<PilotDecision> {
        // Find unvisited alternatives
        let mut alternatives = Vec::new();
        for node_id in state.tree.children(state.tree.root()) {
            if !state.visited.contains(&node_id) {
                alternatives.push(node_id);
            }
        }

        let ranked: Vec<RankedCandidate> = alternatives
            .iter()
            .take(5)
            .map(|&node_id| RankedCandidate::new(node_id, 0.5))
            .collect();

        Some(PilotDecision {
            ranked_candidates: ranked,
            direction: SearchDirection::backtrack("Backtrack guidance", alternatives),
            confidence: 0.5,
            reasoning: "Suggesting alternative branches".to_string(),
            intervention_point: InterventionPoint::Backtrack,
        })
    }

    fn config(&self) -> &PilotConfig {
        &self.config
    }

    fn is_active(&self) -> bool {
        true
    }

    fn reset(&self) {
        // No state to reset
    }
}

fn main() {
    println!("=== Custom Pilot Example ===\n");

    // 1. Create the custom pilot
    let pilot = KeywordPilot::new();
    println!("Created KeywordPilot\n");

    // 2. Create a sample document tree
    let tree = create_sample_tree();
    println!("Created sample tree with {} nodes\n", tree.node_count());

    // 3. Create search state for demonstration
    let query = "What is the architecture?";
    let candidates: Vec<NodeId> = tree.children(tree.root());
    let visited: HashSet<NodeId> = HashSet::new();
    let state = SearchState::new(&tree, query, &[], &candidates, &visited);

    println!("Query: \"{}\"", query);
    println!("Candidates: {}", candidates.len());
    println!("Should intervene: {}\n", pilot.should_intervene(&state));

    // 4. Demonstrate keyword scoring
    println!("Keyword scoring:");
    for node_id in tree.children(tree.root()) {
        if let Some(node) = tree.get(node_id) {
            let score = pilot.score_by_keywords(query, &node.title);
            println!("  - '{}': {:.2}", node.title, score);
        }
    }

    // 5. Show how to integrate with retrieval
    println!("\n--- Integration Example ---\n");
    println!("To use with Engine:");
    println!("```rust");
    println!("use std::sync::Arc;");
    println!("use vectorless::Engine;");
    println!();
    println!("let pilot = Arc::new(KeywordPilot::new());");
    println!("let engine = Engine::builder()");
    println!("    .with_workspace(\"./workspace\")");
    println!("    .with_pilot(pilot)");
    println!("    .build()");
    println!("    .await?;");
    println!("```");

    println!("\n=== Done ===");
}

fn create_sample_tree() -> DocumentTree {
    let mut tree = DocumentTree::new(
        "Vectorless Documentation",
        "A hierarchical document intelligence engine written in Rust.",
    );

    let arch = tree.add_child(
        tree.root(),
        "Architecture",
        "The system consists of three main components.",
    );
    tree.add_child(
        arch,
        "Index Pipeline",
        "Processes documents into a tree structure.",
    );
    tree.add_child(
        arch,
        "Retrieval Pipeline",
        "Finds relevant content using multi-stage processing.",
    );

    let usage = tree.add_child(tree.root(), "Usage", "How to use the vectorless library.");
    tree.add_child(usage, "Basic Example", "Simple usage with default configuration.");
    tree.add_child(
        usage,
        "Advanced Example",
        "Custom pipeline configuration with LLM.",
    );

    tree
}
