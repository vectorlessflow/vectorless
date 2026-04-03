// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Built-in retrieval pipeline stages.
//!
//! This module provides the four core stages for retrieval:
//!
//! - [`AnalyzeStage`] - Query analysis (complexity, keywords, target sections)
//! - [`PlanStage`] - Strategy and algorithm selection
//! - [`SearchStage`] - Execute tree search
//! - [`JudgeStage`] - Sufficiency checking
//!
//! # Stage Flow
//!
//! ```text
//! Analyze → Plan → Search → Judge
//!                    ↑         │
//!                    └─────────┘ (NeedMoreData)
//! ```
//!
//! # Custom Stages
//!
//! Implement [`RetrievalStage`](crate::retrieval::pipeline::RetrievalStage) to create custom stages.

mod analyze;
mod plan;
mod search;
mod judge;

pub use analyze::AnalyzeStage;
pub use plan::PlanStage;
pub use search::SearchStage;
pub use judge::JudgeStage;
