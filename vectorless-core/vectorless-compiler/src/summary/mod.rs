// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Summary generation strategies.
//!
//! This module provides different strategies for generating summaries:
//! - [`SummaryStrategy`] - Configuration for summary generation
//! - [`SummaryStrategyConfig`] - Configuration options
//! - [`SummaryGenerator`] - Trait for summary generation
//! - [`LlmSummaryGenerator`] - LLM-based implementation
//!
//! # Strategies
//!
//! - **None**: No summary generation
//! - **Full**: Generate summaries for all nodes
//! - **Selective**: Generate summaries only for qualifying nodes (default)
//! - **Lazy**: Generate summaries on-demand at query time

mod full;
mod lazy;
mod selective;
mod strategy;

pub use strategy::{LlmSummaryGenerator, SummaryGenerator, SummaryStrategy, SummaryStrategyConfig};
