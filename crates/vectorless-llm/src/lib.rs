// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Unified LLM client module.
//!
//! This module provides a unified interface for all LLM operations across the codebase:
//! - **Compile** — Document compilation and summarization
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
//! │   │   compile   │  │  retrieval  │  │    pilot    │            │
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
pub mod config;
mod error;
mod executor;
mod fallback;
pub mod memo;
mod pool;
pub mod throttle;

pub use client::LlmClient;
pub use error::LlmResult;
pub use pool::LlmPool;
