// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Scoring utilities for text relevance assessment.
//!
//! This module provides text scoring algorithms (BM25, keyword matching)
//! that are used across the retrieval pipeline. These are general-purpose
//! tools, not tied to any specific search algorithm.

pub mod bm25;

pub use bm25::{Bm25Engine, Bm25Params, FieldDocument, STOPWORDS, extract_keywords};
