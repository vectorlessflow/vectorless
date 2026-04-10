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

pub mod cache;
pub mod complexity;
pub mod content;
pub mod pilot;
pub mod pipeline;
pub mod search;
pub mod stages;
pub mod strategy;
pub mod sufficiency;

pub use context::{
    ContextBuilder, PruningStrategy, TokenEstimation, format_for_llm, format_for_llm_async,
    format_tree_for_llm, format_tree_for_llm_async,
};
pub use pipeline_retriever::PipelineRetriever;
pub use retriever::{RetrievalContext, Retriever, RetrieverError, RetrieverResult};
pub use types::*;
pub use types::{LlmCallSummary, ReasoningCandidate, ReasoningChain, ReasoningStep, StageName};

// Re-export StrategyPreference as Strategy for convenience
pub use types::StrategyPreference as Strategy;

// Pipeline exports
pub use pipeline::{
    CandidateNode, ExecutionGroup, FailurePolicy, PipelineContext, RetrievalBudgetController,
    RetrievalMetrics, RetrievalOrchestrator, RetrievalStage, SearchAlgorithm, SearchConfig,
    StageOutcome, BudgetStatus,
};

// Re-export PipelineContext as RetrievalContext for stages (alias for clarity)
pub use pipeline::PipelineContext as StageContext;

// Stage exports
pub use stages::{AnalyzeStage, EvaluateStage, PlanStage, SearchStage};

// Strategy exports
pub use strategy::{
    CrossDocumentConfig, CrossDocumentStrategy, DocumentEntry, DocumentId, DocumentResult,
    HybridConfig, HybridStrategy, KeywordStrategy, LlmStrategy, MergeStrategy,
    PageRange, PageRangeConfig, PageRangeStrategy, RetrievalStrategy, SemanticStrategy,
    StrategyCapabilities, StrategyCost,
};

// Search exports
pub use search::{BeamSearch, GreedySearch, SearchConfig as SearchAlgConfig, SearchResult};

// Sufficiency exports
pub use sufficiency::{SufficiencyChecker, SufficiencyLevel, ThresholdChecker};

// Complexity exports
pub use complexity::ComplexityDetector;

// Cache exports
pub use cache::PathCache;
pub use cache::{CachedCandidate, ReasoningCache, ReasoningCacheConfig, ReasoningCacheStats};

// Content aggregation exports
pub use content::{
    AggregationResult, AllocationResult, AllocationStrategy, BudgetAllocator, ContentAggregator,
    ContentAggregatorConfig, ContentChunk, ContentRelevance, OutputFormat, RelevanceScorer,
    ScoreComponents, ScoringStrategyConfig, SelectedContent, StructureBuilder, StructuredContent,
};

// Pilot exports
pub use pilot::NoopPilot;
pub use pilot::{
    BudgetConfig, InterventionConfig, InterventionPoint, Pilot, PilotConfig, PilotDecision,
    PilotMode, RankedCandidate, SearchDirection, SearchState,
};

// Decompose exports (multi-turn retrieval)
pub use decompose::{
    DecompositionConfig, DecompositionResult, QueryDecomposer, ResultAggregator, SubQuery,
    SubQueryComplexity, SubQueryResult, SubQueryType,
};

// Reference following exports
pub use reference::{
    expand_with_references, FollowedReference, ReferenceConfig, ReferenceExpansion,
    ReferenceFollower,
};

// Streaming exports
pub use stream::{RetrieveEvent, RetrieveEventReceiver, DEFAULT_STREAM_BOUND};
