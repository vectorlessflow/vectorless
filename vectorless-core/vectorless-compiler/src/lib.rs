// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document Compiler module.
//!
//! This module provides a modular document compilation pipeline that transforms
//! documents (Markdown, PDF) into agent-friendly intermediate artifacts.
//!
//!
//! ```text
//! Priority  10: ┌──────────┐
//!               │  Parse    │  Parse document into raw nodes
//!               └────┬─────┘
//! Priority  20: ┌────▼─────┐
//!               │  Build   │  Construct tree + thinning (with content merge)
//!               └────┬─────┘
//! Priority  22: ┌────▼─────┐
//!               │ Validate │  Tree integrity checks (optional)
//!               └────┬─────┘
//! Priority  25: ┌────▼─────┐
//!               │  Split   │  Split oversized leaf nodes (optional)
//!               └────┬─────┘
//! Priority  30: ┌────▼─────┐
//!               │ Enhance  │  LLM summaries (when client available)
//!               └────┬─────┘
//! Priority  40: ┌────▼─────┐
//!               │  Enrich  │  Metadata + cross-references
//!               └────┬─────┘
//! Priority  45: ┌────▼──────────┐
//!               │ Reasoning Idx │  Pre-computed reasoning index
//!               └────┬──────────┘
//! Priority  50: ┌────▼──────────┐
//!               │ Navigation Idx│  Agent navigation index
//!               └────┬──────────┘
//! Priority  60: ┌────▼──────┐
//!               │ Optimize  │  Final tree optimization
//!               └───────────┘
//! ```
//!
//! Checkpointing is available when `PipelineOptions::checkpoint_dir` is set.
//! State is saved after each stage group and resumed on restart.

pub mod config;
pub mod incremental;
pub mod parse;
pub mod pipeline;
pub mod stages;
pub mod summary;

// Re-export main types from pipeline
pub use pipeline::{CompilerInput, IndexMetrics, PipelineExecutor, CompileResult};

// Re-export config types
pub use config::{SourceFormat, PipelineOptions, ThinningConfig};
pub use vectorless_document::ReasoningIndexConfig;

// Re-export summary
pub use summary::SummaryStrategy;
