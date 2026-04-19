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
//!   → extract keywords            (from utils/bm25)
//!   → QueryPlan
//! ```
//!
//! Future additions (not yet implemented):
//! - Intent classification (`QueryIntent`)
//! - Query rewrite / expansion
//! - Multi-query decomposition

mod text;
mod types;
