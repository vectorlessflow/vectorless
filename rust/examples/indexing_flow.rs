// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Indexing pipeline flow example — demonstrates the full indexing pipeline
//! with detailed metrics breakdown.
//!
//! This example walks through:
//! 1. Creating a Vectorless engine
//! 2. Indexing a Markdown document from content
//! 3. Inspecting per-stage timing metrics
//!
//! Set `RUST_LOG=info` to see pipeline stage logs, or `RUST_LOG=debug` for
//! detailed internal progress.
//!
//! # Usage
//!
//! ```bash
//! # Using environment variables for LLM config:
//! LLM_API_KEY=sk-xxx LLM_MODEL=google/gemini-3-flash-preview \
//!   LLM_ENDPOINT=http://localhost:4000/api/v1 cargo run --example indexing_flow
//!
//! # Or with defaults (edit the code to set your key/endpoint):
//! cargo run --example indexing_flow
//! ```

use vectorless::{DocumentFormat, EngineBuilder, IndexContext};

/// Sample document with multi-level headings to exercise tree construction
/// and navigation index building.
const SAMPLE_MARKDOWN: &str = r#"
# Payment Platform Technical Guide

## Overview

This guide covers the architecture and implementation details of the payment processing platform. The system handles credit card payments, bank transfers, and digital wallets across multiple currencies and regions. It is designed for high availability with 99.99% uptime SLA and supports peak throughput of 10,000 transactions per second.

## Architecture

The platform uses a microservices architecture with event-driven communication between services. Each service owns its data store and communicates through a message broker for eventual consistency. The system is deployed on Kubernetes with automatic horizontal scaling based on request queue depth.

### Ingestion Gateway

The ingestion gateway is the entry point for all payment requests. It handles request validation, authentication, idempotency checks, and routing to the appropriate payment processor. The gateway implements circuit breaker patterns to gracefully degrade when downstream processors experience issues.

### Payment Processing Engine

The payment processing engine orchestrates the lifecycle of each payment transaction. It manages state transitions from initiation through authorization, capture, settlement, and reconciliation. The engine supports both synchronous and asynchronous payment flows, depending on the payment method and processor requirements.

### Settlement Service

The settlement service handles batch settlement with acquiring banks and payment networks. It runs on a configurable schedule (typically end-of-day for each banking region) and groups authorized transactions into settlement batches. The service handles currency conversion, fee calculation, and split payments for marketplace scenarios.

## Security

All payment data is encrypted at rest using AES-256 and in transit using TLS 1.3. Cardholder data is tokenized immediately upon receipt and stored in a PCI DSS Level 1 compliant vault. The platform undergoes annual PCI DSS audits and quarterly network vulnerability scans.

### Fraud Detection

Real-time fraud detection uses a rules engine combined with a machine learning model that scores each transaction based on velocity checks, geolocation anomalies, device fingerprinting, and behavioral patterns. Transactions exceeding configurable risk thresholds are automatically held for manual review.

### Compliance

The platform complies with PCI DSS, SOC 2 Type II, GDPR, and regional payment regulations including PSD2 (Europe) and local data residency requirements. Audit logs are retained for 7 years and accessible through a dedicated compliance API.

## Monitoring and Operations

Real-time dashboards track transaction volumes, success rates, latency percentiles, and error rates across all payment methods and processors. Automated alerting triggers on-call rotations when key metrics deviate from baseline thresholds.
"#;

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    tracing_subscriber::fmt::init();

    println!("=== Indexing Pipeline Flow Example ===\n");

    // Build engine with LLM configuration from environment or defaults.
    let api_key = std::env::var("LLM_API_KEY").unwrap_or_else(|_| "sk-...".to_string());
    let model =
        std::env::var("LLM_MODEL").unwrap_or_else(|_| "google/gemini-3-flash-preview".to_string());
    let endpoint = std::env::var("LLM_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4000/api/v1".to_string());

    // Step 1: Create engine
    println!("Step 1: Creating engine...");
    let engine = EngineBuilder::new()
        .with_key(&api_key)
        .with_model(&model)
        .with_endpoint(&endpoint)
        .build()
        .await
        .map_err(|e| vectorless::Error::Config(e.to_string()))?;
    println!("  Done.\n");

    // Step 2: Index from content
    println!("Step 2: Indexing document from content...\n");
    let result = engine
        .index(IndexContext::from_content(
            SAMPLE_MARKDOWN,
            DocumentFormat::Markdown,
        ))
        .await?;

    println!("  Indexed {} document(s)\n", result.items.len());

    // Step 3: Inspect indexing results and metrics
    for item in &result.items {
        println!("--- Document Info ---");
        println!("  doc_id:    {}", item.doc_id);
        println!("  name:      {}", item.name);
        println!("  format:    {:?}", item.format);

        if let Some(desc) = &item.description {
            println!(
                "  summary:   {}...",
                &desc[..desc.len().min(120)]
            );
        }

        if let Some(ref metrics) = item.metrics {
            println!("\n--- Pipeline Stage Metrics ---");
            println!("  Stage                Time (ms)");
            println!("  ─────────────────────────────");
            println!("  Parse              {:>8}", metrics.parse_time_ms);
            println!("  Build              {:>8}", metrics.build_time_ms);
            println!("  Validate           {:>8}", metrics.validate_time_ms);
            println!("  Split              {:>8}", metrics.split_time_ms);
            println!("  Enhance            {:>8}", metrics.enhance_time_ms);
            println!("  Enrich             {:>8}", metrics.enrich_time_ms);
            println!("  Reasoning Index    {:>8}", metrics.reasoning_index_time_ms);
            println!("  Navigation Index   {:>8}", metrics.navigation_index_time_ms);
            println!("  Optimize           {:>8}", metrics.optimize_time_ms);
            println!("  ─────────────────────────────");
            println!("  Total              {:>8}", metrics.total_time_ms());

            println!("\n--- Index Output ---");
            println!("  Nodes processed:       {}", metrics.nodes_processed);
            println!("  Summaries generated:   {}", metrics.summaries_generated);
            println!("  Summaries failed:      {}", metrics.summaries_failed);
            println!("  LLM calls:             {}", metrics.llm_calls);
            println!("  Tokens generated:      {}", metrics.total_tokens_generated);

            println!("\n--- Navigation Index ---");
            println!("  Nav entries:           {}", metrics.nav_entries_indexed);
            println!("  Child routes:          {}", metrics.child_routes_indexed);

            println!("\n--- Reasoning Index ---");
            println!("  Topics indexed:        {}", metrics.topics_indexed);
            println!("  Keywords indexed:      {}", metrics.keywords_indexed);

            println!("\n--- Tree Optimization ---");
            println!("  Nodes skipped:         {}", metrics.nodes_skipped);
            println!("  Nodes merged:          {}", metrics.nodes_merged);
        }

        println!();
    }

    // Step 4: Cleanup
    println!("Step 3: Cleaning up...");
    for doc in engine.list().await? {
        engine.remove(&doc.id).await?;
        println!("  Removed: {} ({})", doc.name, doc.id);
    }

    println!("\n=== Done ===");
    Ok(())
}
