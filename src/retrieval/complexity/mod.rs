// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Query complexity detection.
//!
//! Determines the complexity level of a query for adaptive strategy selection.

mod detector;

pub use super::types::QueryComplexity;
pub use detector::ComplexityDetector;
