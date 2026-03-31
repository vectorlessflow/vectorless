// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document summarization module.
//!
//! This module provides summarization strategies for document content:
//! - **LLM-based** - Uses language models for abstractive summarization

mod llm;

// Re-export from core traits
pub use crate::core::SummarizerConfig;

// Re-export LLM utilities
pub use llm::{summarize, LlmError};
