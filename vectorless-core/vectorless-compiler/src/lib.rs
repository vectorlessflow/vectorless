// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document Compiler module.
//!
//! This module provides a modular document compilation pipeline that transforms
//! documents (Markdown, PDF) into agent-friendly intermediate artifacts.
//!
//! ```text
//! Frontend  10: ┌──────────┐
//!               │  Parse    │  Parse document into raw nodes
//!               └────┬─────┘
//! Frontend  20: ┌────▼─────┐
//!               │  Build   │  Construct tree + thinning
//!               └────┬─────┘
//! Analysis  22: ┌────▼─────┐
//!               │ Validate │  Tree integrity checks (optional)
//!               └────┬─────┘
//! Transform 25: ┌────▼─────┐
//!               │  Split   │  Split oversized leaf nodes (optional)
//!               └────┬─────┘
//! Analysis  30: ┌────▼─────┐
//!               │ Enhance  │  LLM summaries
//!               └────┬─────┘
//! Transform 40: ┌────▼─────┐
//!               │  Enrich  │  Metadata + cross-references
//!               └────┬─────┘
//! Backend   45: ┌────▼──────────┐
//!               │ Reasoning Idx │  Symbol table (keyword→path mapping)
//!               └────┬──────────┘
//! Backend   47: ┌────▼──────────┐
//!               │ Concept       │  Concept extraction
//!               └────┬──────────┘
//! Backend   50: ┌────▼──────────┐
//!               │ Navigation Idx│  Debug info for runtime navigation
//!               └────┬──────────┘
//! Backend   55: ┌────▼──────┐
//!               │  Verify   │  Output validation
//!               └────┬──────┘
//! Backend   60: ┌────▼──────┐
//!               │ Optimize  │  Final tree optimization
//!               └───────────┘
//! ```
//!
//! Checkpointing is available when `PipelineOptions::checkpoint_dir` is set.
//! State is saved after each pass group and resumed on restart.

pub mod config;
pub mod incremental;
pub mod parse;
pub mod pipeline;
pub mod passes;
pub mod summary;

// Re-export main types from pipeline
pub use pipeline::{CompilerInput, IndexMetrics, PipelineExecutor, CompileResult};

// Re-export config types
pub use config::{SourceFormat, PipelineOptions, ThinningConfig};
pub use vectorless_document::ReasoningIndexConfig;

// Re-export summary
pub use summary::SummaryStrategy;
