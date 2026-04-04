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
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                              Pilot Architecture                          │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                  │
//! │   │   Query     │   │  Context    │   │  Decision   │                  │
//! │   │  Analyzer   │──▶│   Builder   │──▶│   Engine    │                  │
//! │   └─────────────┘   └─────────────┘   └──────┬──────┘                  │
//! │                                              │                          │
//! │                                              ▼                          │
//! │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                  │
//! │   │   Response  │◀──│     LLM     │◀──│   Prompt    │                  │
//! │   │   Parser    │   │   Client    │   │   Builder   │                  │
//! │   └─────────────┘   └─────────────┘   └─────────────┘                  │
//! │                                                                         │
//! │   Supporting: BudgetController, FallbackManager, MetricsCollector      │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
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

mod config;
mod decision;
mod r#trait;

pub use config::{
    BudgetConfig, InterventionConfig, PilotConfig, PilotMode,
};
pub use decision::{
    InterventionPoint, PilotDecision, RankedCandidate, SearchDirection,
};
pub use r#trait::{Pilot, SearchState};

// Re-export for convenience
#[cfg(feature = "llm-pilot")]
mod llm_pilot;

#[cfg(feature = "llm-pilot")]
pub use llm_pilot::LlmPilot;

/// NoopPilot - A no-op implementation for testing and fallback.
mod noop;
pub use noop::NoopPilot;
