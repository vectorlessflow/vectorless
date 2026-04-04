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
//! final_score = α × algorithm_score + β × llm_score
//! ```
//!
//! # TODO: Implementation steps
//!
//! 1. Define your custom Pilot struct
//! 2. Implement the Pilot trait
//! 3. Configure intervention conditions
//! 4. Integrate with EngineBuilder

// TODO: Implement custom Pilot
// ```
// use vectorless::retrieval::pilot::{Pilot, PilotDecision, SearchState, InterventionPoint};
//
// pub struct MyCustomPilot {
//     // Your fields here
// }
//
// impl Pilot for MyCustomPilot {
//     fn should_intervene(&self, state: &SearchState, point: InterventionPoint) -> bool {
//         // Decide when to intervene
//         todo!()
//     }
//
//     async fn decide(&self, state: &SearchState) -> PilotDecision {
//         // Make navigation decision
//         todo!()
//     }
// }
// ```

fn main() {
    // TODO: Show how to use custom Pilot with EngineBuilder
    //
    // let pilot = MyCustomPilot::new();
    // let engine = EngineBuilder::new()
    //     .with_pilot(Arc::new(pilot))
    //     .build()?;
    //
    // // Use engine with custom Pilot guidance

    println!("TODO: Implement custom_pilot example");
}
