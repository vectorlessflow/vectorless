// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Utility functions and helpers.
//!
//! This module provides common utilities used across the codebase:
//!
//! - **Token estimation** — Fast and accurate token counting
//! - **Timing** — Performance measurement utilities
//! - **Format** — Text and number formatting utilities

mod format;
mod timing;
mod token;

pub use format::{
    clean_whitespace, format_bytes, format_number, format_percent, indent, line_count, truncate,
    truncate_words, word_count,
};
pub use timing::{Timer, format_duration, format_duration_compact};
pub use token::{estimate_tokens, estimate_tokens_batch, estimate_tokens_fast};
