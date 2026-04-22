// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Unified token estimation module.
//!
//! Provides accurate token counting using tiktoken for OpenAI models,
//! with fallback to character-based estimation for other models.

use std::sync::OnceLock;
use tiktoken_rs::CoreBPE;

/// Global BPE encoder instance (cl100k_base is used by GPT-4, GPT-3.5-turbo, text-embedding-ada-002)
static BPE: OnceLock<CoreBPE> = OnceLock::new();

/// Get or initialize the BPE encoder.
fn get_bpe() -> &'static CoreBPE {
    BPE.get_or_init(|| {
        tiktoken_rs::cl100k_base().expect("Failed to initialize cl100k_base tokenizer")
    })
}

/// Estimate token count for a text using tiktoken.
///
/// This uses the cl100k_base encoding which is used by:
/// - GPT-4
/// - GPT-3.5-turbo
/// - GPT-4o
/// - GPT-4o-mini
/// - text-embedding-ada-002
/// - text-embedding-3-small/large
///
/// # Example
///
/// ```
/// use vectorless::estimate_tokens;
///
/// assert_eq!(estimate_tokens(""), 0);
/// assert!(estimate_tokens("hello world") > 0);
/// ```
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    // Use tiktoken for accurate counting
    get_bpe().encode_with_special_tokens(text).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_simple() {
        // "hello world" should be 2 tokens with tiktoken
        let count = estimate_tokens("hello world");
        assert!(count >= 2, "Expected at least 2 tokens, got {}", count);
    }
}
