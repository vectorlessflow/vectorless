// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Scoring utilities — keyword extraction via BM25.

pub mod bm25;

pub use bm25::extract_keywords;
