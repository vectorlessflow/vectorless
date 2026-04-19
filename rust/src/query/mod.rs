// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Query understanding and planning.
//!
//! This module is responsible for analyzing a user's raw query and producing
//! a structured [`QueryPlan`] that downstream modules (retrieval, agent) can
//! consume. It does **not** perform any retrieval itself.
//!
//! # Pipeline
//!
//! ```text
//! raw query string
//!   → detect_query_complexity()   (heuristic, zero-cost)
//!   → extract keywords            (from utils/bm25)
//!   → compute adaptive budget     (complexity × document depth)
//!   → QueryPlan
//! ```
//!
//! Future additions (not yet implemented):
//! - Intent classification (`QueryIntent`)
//! - Query rewrite / expansion
//! - Multi-query decomposition

mod budget;
mod complexity;
mod text;
mod types;

pub use budget::Budget;
pub use complexity::detect_query_complexity;
pub use types::{QueryComplexity, QueryPlan};
