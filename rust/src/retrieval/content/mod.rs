// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Content aggregation module for retrieval results.
//!
//! This module provides precision-focused, budget-aware content aggregation
//! that transforms candidate nodes into structured, relevant content.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Content Aggregator                         │
//! ├─────────────────────────────────────────────────────────────┤
//! │  RelevanceScorer → BudgetAllocator → StructureBuilder       │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::retrieval::content::{ContentAggregator, ContentAggregatorConfig};
//!
//! let config = ContentAggregatorConfig {
//!     token_budget: 4000,
//!     min_relevance_score: 0.3,
//!     ..Default::default()
//! };
//!
//! let aggregator = ContentAggregator::new(config);
//! let result = aggregator.aggregate(&candidates, &tree, &query);
//! ```

mod aggregator;
mod budget;
mod builder;
mod config;
mod scorer;

pub use aggregator::{AggregationResult, CandidateNode, ContentAggregator};
pub use budget::{AllocationResult, AllocationStrategy, BudgetAllocator, SelectedContent};
pub use builder::{ContentTree, OutputFormat, StructureBuilder, StructuredContent};
pub use config::{ContentAggregatorConfig, OutputFormatConfig, ScoringStrategyConfig};
pub use scorer::{
    ContentChunk, ContentRelevance, RelevanceScorer, ScoreComponents, ScoringContext,
};
