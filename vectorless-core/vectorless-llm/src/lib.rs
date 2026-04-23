// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Unified LLM client module.
//!
//! This module provides a unified interface for all LLM operations across the codebase:
//! - **Index** — Document indexing and summarization
//! - **Retrieval** — Document tree navigation
//! - **Pilot** — Navigation guidance
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        LlmPool                                   │
//! │                                                                  │
//! │   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │
//! │   │    index    │  │  retrieval  │  │    pilot    │            │
//! │   │  LlmClient  │  │  LlmClient  │  │  LlmClient  │            │
//! │   └──────┬──────┘  └──────┬──────┘  └──────┬──────┘            │
//! │          │                │                │                   │
//! │          └────────────────┼────────────────┘                   │
//! │                           │                                    │
//! │                           ▼                                    │
//! │               ┌─────────────────────┐                          │
//! │               │   async-openai      │                          │
//! │               └─────────────────────┘                          │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

mod client;
pub(crate) mod config;
mod error;
mod executor;
mod fallback;
pub(crate) mod memo;
mod pool;
pub(crate) mod throttle;

pub use client::LlmClient;
pub use error::LlmResult;
pub use pool::LlmPool;

// Re-export vectorless_error types for internal use
pub(crate) use vectorless_error::{Error, Result};
