// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pilot - The brain of the Retriever Pipeline.
//!
//! Pilot is the core intelligence component responsible for understanding queries,
//! analyzing document structure, and making navigation decisions. Unlike traditional
//! vector-based retrieval, Pilot uses LLM for semantic understanding and navigation
//! while keeping the algorithm efficient for execution.
//!
//! # Design Philosophy
//!
//! 1. Algorithm handles "how to search" - efficient, deterministic, low latency
//! 2. Pilot handles "where to go" - semantic understanding, disambiguation, direction
//! 3. Intervention at key decision points - not every step, only when needed
//! 4. Layered fallback - algorithm takes over when LLM fails, Pilot rescues when algorithm fails
//!
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use vectorless::retrieval::pilot::{LlmPilot, PilotConfig, Pilot};
//!
//! let pilot = LlmPilot::new(llm_client, PilotConfig::default());
//!
//! // Check if intervention needed
//! if pilot.should_intervene(&state) {
//!     let decision = pilot.decide(&state).await;
//!     // Use decision to guide search
//! }
//! ```

mod budget;
mod builder;
mod complexity;
mod config;
mod decision;
mod decision_scorer;
mod fallback;
mod feedback;
mod llm_pilot;
mod metrics;
mod noop;
mod parser;
mod prompts;
mod scorer;
mod r#trait;

pub use complexity::detect_with_llm;
pub use config::PilotConfig;
pub use decision::{InterventionPoint, PilotDecision};
pub use decision_scorer::{PilotDecisionCache, score_candidates, score_candidates_detailed};
pub use llm_pilot::LlmPilot;
pub use scorer::{NodeScorer, ScoringContext};
pub use r#trait::{Pilot, SearchState};
