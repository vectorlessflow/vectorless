// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Summary generation strategies.
//!
//! This module provides different strategies for generating summaries:
//! - [`SummaryStrategy`] - Configuration for summary generation
//! - [`SummaryStrategyConfig`] - Configuration options
//! - [`SummaryGenerator`] - Trait for summary generation
//! - [`LlmSummaryGenerator`] - LLM-based implementation

mod strategy;

pub use strategy::{SummaryStrategy, SummaryStrategyConfig, SummaryGenerator, LlmSummaryGenerator};
