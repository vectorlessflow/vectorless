// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Concurrency control for LLM API calls.
//!
//! This module provides rate limiting and concurrency control to prevent
//! overwhelming LLM API endpoints:
//!
//! - **Rate Limiter** — Token bucket algorithm to limit requests per time period
//! - **Concurrency Controller** — Combined semaphore + rate limiter
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        LlmClient                                 │
//! │                                                                  │
//! │   complete() ──▶ [Rate Limiter] ──▶ [Semaphore] ──▶ API Call   │
//! │                         │                │                       │
//! │                    令牌桶限制        并发数限制                   │
//! │                                                                  │
//! │   ┌─────────────────────────────────────────────────────────┐  │
//! │   │              ConcurrencyController                       │  │
//! │   │                                                          │  │
//! │   │  ┌─────────────┐  ┌─────────────┐                        │  │
//! │   │  │RateLimiter  │  │ Semaphore   │                        │  │
//! │   │  │(governor)   │  │(tokio)      │                        │  │
//! │   │  └─────────────┘  └─────────────┘                        │  │
//! │   └─────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust
//! use vectorless::throttle::{ConcurrencyController, ConcurrencyConfig};
//!
//! # #[tokio::main]
//! # async fn main() {
//! // Create with default configuration
//! let controller = ConcurrencyController::with_defaults();
//!
//! // Or customize
//! let config = ConcurrencyConfig::new()
//!     .with_max_concurrent_requests(20)
//!     .with_requests_per_minute(1000);
//! let controller = ConcurrencyController::new(config);
//!
//! // Before making an API call
//! let permit = controller.acquire().await;
//!
//! // Make the API call...
//! // Permit is automatically released when dropped
//! # }
//! ```

mod config;
mod rate_limiter;
mod controller;

pub use config::ConcurrencyConfig;
pub use rate_limiter::RateLimiter;
pub use controller::ConcurrencyController;
