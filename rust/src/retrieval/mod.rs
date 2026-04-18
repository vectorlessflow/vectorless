// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval system for Vectorless document trees.
//!
//! This module implements a hybrid retrieval architecture combining:
//! - **Adaptive Strategy Selection**: Automatically chooses between keyword, semantic, and LLM strategies
//! - **Multi-Path Search**: Beam search and MCTS for exploring multiple tree paths
//! - **Incremental Retrieval**: Stops early when sufficient information is collected
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    RetrievalOrchestrator                         │
//! │                                                                  │
//! │  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐      │
//! │  │ Analyze │───►│  Plan   │───►│ Search  │───►│  Evaluate  │      │
//! │  └─────────┘    └─────────┘    └─────────┘    └─────────┘      │
//! │                                     ▲              │             │
//! │                                     └──────────────┘             │
//! │                                    (NeedMoreData)               │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Pipeline Stages
//!
//! | Stage | Description |
//! |-------|-------------|
//! | [`AnalyzeStage`] | Query analysis (complexity, keywords, targets) |
//! | [`PlanStage`] | Strategy and algorithm selection |
//! | [`SearchStage`] | Execute tree search |
//! | [`EvaluateStage`] | Sufficiency checking |
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use vectorless::retrieval::pipeline::{RetrievalOrchestrator, RetrievalStage};
//! use vectorless::retrieval::stages::{AnalyzeStage, PlanStage, SearchStage, EvaluateStage};
//!
//! let orchestrator = RetrievalOrchestrator::new()
//!     .stage(AnalyzeStage::new())
//!     .stage(PlanStage::new())
//!     .stage(SearchStage::new())
//!     .stage(EvaluateStage::new());
//!
//! let response = orchestrator.execute(tree, query, options).await?;
//! ```

mod context;
mod decompose;
mod pipeline_retriever;
mod reference;
mod retriever;
pub mod stream;
mod types;

pub mod agent;
pub mod cache;
pub mod complexity;
pub mod content;
pub mod pilot;
pub mod pipeline;
pub mod scoring;
pub mod search;
pub mod stages;
pub mod strategy;
pub mod sufficiency;

pub use context::{PruningStrategy, TokenEstimation};
pub use pipeline_retriever::PipelineRetriever;
pub use retriever::RetrievalContext;
pub use types::*;

// Sufficiency exports
pub use sufficiency::SufficiencyLevel;

// Streaming exports
pub use stream::RetrieveEventReceiver;
