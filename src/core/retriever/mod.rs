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
//! ┌─────────────────────────────────────────────────────────┐
//! │                    AdaptiveRetriever                     │
//! │  ┌─────────────────────────────────────────────────┐    │
//! │  │              Complexity Detector                 │    │
//! │  │   Simple ─────────► Medium ─────────► Complex   │    │
//! │  └─────────────────────────────────────────────────┘    │
//! │                          │                               │
//! │  ┌───────────┬───────────┴───────────┬───────────┐      │
//! │  │ Keyword   │      Semantic         │    LLM    │      │
//! │  │ Strategy  │      Strategy         │  Strategy │      │
//! │  └───────────┴───────────────────────┴───────────┘      │
//! │                          │                               │
//! │  ┌───────────────────────┴───────────────────────┐      │
//! │  │              Multi-Path Search                 │      │
//! │  │   Greedy │ Beam Search │ MCTS                 │      │
//! │  └───────────────────────────────────────────────┘      │
//! │                          │                               │
//! │  ┌───────────────────────┴───────────────────────┐      │
//! │  │           Sufficiency Checker                  │      │
//! │  │   Threshold-based │ LLM-based Judge           │      │
//! │  └───────────────────────────────────────────────┘      │
//! └─────────────────────────────────────────────────────────┘
//! ```

mod types;
mod retriever;
mod adaptive;
mod context;

pub mod strategy;
pub mod search;
pub mod sufficiency;
pub mod complexity;
pub mod cache;

pub use types::*;
pub use retriever::{Retriever, RetrieverError, RetrieverResult, RetrievalContext};
pub use adaptive::{AdaptiveRetriever, AdaptiveConfig};
pub use context::{ContextBuilder, format_for_llm, format_tree_for_llm};
pub use strategy::{RetrievalStrategy, StrategyCapabilities, KeywordStrategy, SemanticStrategy, LlmStrategy};
pub use search::{BeamSearch, GreedySearch, SearchConfig, SearchResult};
pub use sufficiency::{SufficiencyChecker, SufficiencyLevel, ThresholdChecker};
pub use complexity::ComplexityDetector;
pub use cache::PathCache;
