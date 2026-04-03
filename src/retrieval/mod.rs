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
//! │  │ Analyze │───►│  Plan   │───►│ Search  │───►│  Judge  │      │
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
//! | [`JudgeStage`] | Sufficiency checking |
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use vectorless::retrieval::pipeline::{RetrievalOrchestrator, RetrievalStage};
//! use vectorless::retrieval::stages::{AnalyzeStage, PlanStage, SearchStage, JudgeStage};
//!
//! let orchestrator = RetrievalOrchestrator::new()
//!     .stage(AnalyzeStage::new())
//!     .stage(PlanStage::new())
//!     .stage(SearchStage::new())
//!     .stage(JudgeStage::new());
//!
//! let response = orchestrator.execute(tree, query, options).await?;
//! ```

mod types;
mod retriever;
mod context;
mod pipeline_retriever;

pub mod pipeline;
pub mod stages;
pub mod strategy;
pub mod search;
pub mod sufficiency;
pub mod complexity;
pub mod cache;

pub use types::*;
pub use retriever::{Retriever, RetrieverError, RetrieverResult, RetrievalContext};
pub use pipeline_retriever::PipelineRetriever;
pub use context::{
    ContextBuilder, PruningStrategy, TokenEstimation,
    format_for_llm, format_for_llm_async,
    format_tree_for_llm, format_tree_for_llm_async,
};

// Pipeline exports
pub use pipeline::{
    RetrievalOrchestrator, RetrievalStage, PipelineContext,
    StageOutcome, ExecutionGroup, FailurePolicy,
    CandidateNode, SearchAlgorithm, SearchConfig, RetrievalMetrics,
};

// Re-export PipelineContext as RetrievalContext for stages (alias for clarity)
pub use pipeline::PipelineContext as StageContext;

// Stage exports
pub use stages::{AnalyzeStage, PlanStage, SearchStage, JudgeStage};

// Strategy exports
pub use strategy::{RetrievalStrategy, StrategyCapabilities, KeywordStrategy, SemanticStrategy, LlmStrategy};

// Search exports
pub use search::{BeamSearch, GreedySearch, SearchConfig as SearchAlgConfig, SearchResult};

// Sufficiency exports
pub use sufficiency::{SufficiencyChecker, SufficiencyLevel, ThresholdChecker};

// Complexity exports
pub use complexity::ComplexityDetector;

// Cache exports
pub use cache::PathCache;
