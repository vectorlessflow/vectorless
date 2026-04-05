// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Built-in retrieval pipeline stages.
//!
//! This module provides the four core stages for retrieval:
//!
//! - [`AnalyzeStage`] - Query analysis (complexity, keywords, target sections)
//! - [`PlanStage`] - Strategy and algorithm selection
//! - [`SearchStage`] - Execute tree search
//! - [`EvaluateStage`] - Sufficiency checking
//!
//! # Stage Flow
//!
//! ```text
//! Analyze → Plan → Search → Evaluate
//!                    ↑         │
//!                    └─────────┘ (NeedMoreData)
//! ```
//!
//! # Custom Stages
//!
//! Implement [`RetrievalStage`](crate::retrieval::pipeline::RetrievalStage) to create custom stages.

mod analyze;
mod evaluate;
mod plan;
mod search;

pub use analyze::AnalyzeStage;
pub use evaluate::EvaluateStage;
pub use plan::PlanStage;
pub use search::SearchStage;
