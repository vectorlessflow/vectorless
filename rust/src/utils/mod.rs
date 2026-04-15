// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Utility functions and helpers.
//!
//! This module provides common utilities used across the codebase:
//!
//! - **Token estimation** — Fast and accurate token counting
//! - **Timing** — Performance measurement utilities
//! - **Format** — Text and number formatting utilities

pub mod fingerprint;
mod format;
mod timing;
mod token;
pub mod validation;

pub use token::estimate_tokens;
pub use validation::{validate_bytes, validate_content, validate_file};
