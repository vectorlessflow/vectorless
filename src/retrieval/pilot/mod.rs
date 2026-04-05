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

mod budget;
mod builder;
mod config;
mod decision;
mod fallback;
mod feedback;
mod llm_pilot;
mod metrics;
mod noop;
mod parser;
mod prompts;
mod r#trait;

pub use budget::{BudgetController, BudgetUsage};
pub use builder::{ContextBuilder, PilotContext, TokenBudget};
pub use config::{BudgetConfig, InterventionConfig, PilotConfig, PilotMode};
pub use decision::{InterventionPoint, PilotDecision, RankedCandidate, SearchDirection};
pub use fallback::{FallbackAction, FallbackConfig, FallbackError, FallbackLevel, FallbackManager};
pub use feedback::{
    ContextStats, DecisionAdjustment, DecisionId, FeedbackId, FeedbackRecord, FeedbackStore,
    FeedbackStoreConfig, InterventionStats, LearnerConfig, PilotLearner,
};
pub use llm_pilot::LlmPilot;
pub use metrics::{CallRecord, MetricsCollector, PilotMetrics};
pub use noop::NoopPilot;
pub use parser::ResponseParser;
pub use prompts::PromptBuilder;
pub use r#trait::{Pilot, PilotExt, SearchState};
