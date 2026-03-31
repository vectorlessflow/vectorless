// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Result ranking and merging module.
//!
//! This module provides:
//! - **Scoring** — Relevance scoring strategies
//! - **Merging** — Deduplication and result combination
//!
//! # Example
//!
//! ```rust
//! use vectorless::ranking::{Scorer, Merger, ScoredResult};
//! use vectorless::retriever::RetrievalResult;
//!
//! let results = vec![
//!     RetrievalResult::new("Section 1").with_score(0.8),
//!     RetrievalResult::new("Section 2").with_score(0.6),
//! ];
//!
//! // Score results
//! let scorer = Scorer::new();
//! let scored = scorer.score(&results, "query");
//!
//! // Merge and deduplicate
//! let merger = Merger::new();
//! let merged = merger.merge(scored, 0.7);
//! ```

mod scorer;
mod merger;

pub use scorer::{Scorer, ScoringStrategy, ScoredResult};
pub use merger::{Merger, MergeStrategy};
