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
mod pool;

pub use client::LlmClient;
pub use error::LlmResult;
pub use executor::LlmExecutor;
pub use pool::LlmPool;
