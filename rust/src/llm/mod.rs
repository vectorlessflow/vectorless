// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Unified LLM client module.
//!
//! This module provides a unified interface for all LLM operations across the codebase:
//! - **Index** — Document indexing and summarization
//! - **Retrieval** — Document tree navigation
//! - **Pilot** — Navigation guidance
//!
//! # Features
//!
//! - Unified configuration with purpose-specific presets
//! - Automatic retry with exponential backoff
//! - JSON response parsing
//! - Unified error handling
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
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::llm::{LlmPool, LlmConfig, RetryConfig};
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::llm::LlmResult<()> {
//! // Create a pool with default configurations
//! let pool = LlmPool::from_defaults();
//!
//! // Use index client
//! let summary = pool.index().complete(
//!     "You summarize text concisely.",
//!     "Long text to summarize..."
//! ).await?;
//!
//! // Use retrieval client with JSON output
//! #[derive(serde::Deserialize)]
//! struct NavDecision { section: usize }
//! let decision: NavDecision = pool.retrieval().complete_json(
//!     "You navigate documents.",
//!     "Find section about X..."
//! ).await?;
//!
//! # Ok(())
//! # }
//! ```

mod client;
mod config;
mod error;
mod executor;
mod fallback;
mod pool;
mod retry;

pub use client::LlmClient;
pub use config::LlmConfigs;
pub use error::LlmResult;
pub use executor::LlmExecutor;
pub use pool::LlmPool;
