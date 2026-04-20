// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Unified metrics collection for Vectorless.
//!
//! This module provides centralized metrics collection across all components:
//! - **LLM Metrics** — Token usage, latency, cost
//! - **Retrieval Metrics** — Paths, scores, iterations, cache
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        MetricsHub                                │
//! │                                                                  │
//! │   ┌─────────────┐   ┌──────────────────┐                       │
//! │   │  LlmMetrics │   │RetrievalMetrics  │                       │
//! │   │             │   │                  │                       │
//! │   │ - tokens    │   │ - paths          │                       │
//! │   │ - latency   │   │ - scores         │                       │
//! │   │ - cost      │   │ - cache          │                       │
//! │   └─────────────┘   └──────────────────┘                       │
//! │                                                                  │
//! │   ┌─────────────────────────────────────────────────────────┐  │
//! │   │                    MetricsReport                         │  │
//! │   │                                                         │  │
//! │   │   Aggregated report with all metrics and statistics     │  │
//! │   └─────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust
//! use vectorless::metrics::{MetricsHub, MetricsConfig};
//!
//! let config = MetricsConfig::default();
//! let hub = MetricsHub::new(config);
//!
//! // Record LLM call
//! hub.record_llm_call(100, 50, 150, true);
//!
//! // Generate report
//! let report = hub.generate_report();
//! println!("Total cost: ${:.4}", report.llm.estimated_cost_usd);
//! ```

mod hub;
mod index;
mod llm;
mod retrieval;

pub use hub::{MetricsHub, MetricsReport};
pub use index::IndexMetrics;
pub use llm::LlmMetricsReport;
pub use retrieval::RetrievalMetricsReport;
