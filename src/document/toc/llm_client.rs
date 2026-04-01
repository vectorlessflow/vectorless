// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM client utilities for TOC processing.
//!
//! This module re-exports the unified [`crate::llm`] types for backward
//! compatibility with existing TOC code.

// Re-export from unified llm module
pub use crate::llm::{LlmClient, LlmConfig, LlmError, LlmResult};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = LlmClient::with_defaults();
        assert!(client.config().model.len() > 0);
    }
}
