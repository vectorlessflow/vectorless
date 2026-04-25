// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Utility functions and helpers.
//!
//! This module provides common utilities used across the codebase:
//!
//! - **Token estimation** — Fast and accurate token counting (tiktoken-based)
//! - **Fingerprint** — BLAKE2b content hashing for change detection
//! - **Validation** — Pre-index source validation (file, content, bytes)

pub mod fingerprint;
mod token;
pub mod validation;

pub use token::estimate_tokens;
pub use validation::{validate_bytes, validate_content, validate_file};
