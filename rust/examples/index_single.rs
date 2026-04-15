// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Single document indexing example — index one document from content.
//!
//! ```bash
//! # Using environment variables for LLM config:
//! LLM_API_KEY=sk-xxx LLM_MODEL=google/gemini-3-flash-preview \
//!   LLM_ENDPOINT=http://localhost:4000/api/v1 cargo run --example index_single
//!
//! # Or with defaults (edit the code to set your key/endpoint):
//! cargo run --example index_single
//! ```

use vectorless::{DocumentFormat, EngineBuilder, IndexContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    // Initialize tracing for debug output (set RUST_LOG=debug to see more)
    tracing_subscriber::fmt::init();

    // Build engine with LLM configuration from environment or defaults.
    // Adjust the defaults below to match your setup.
    let api_key = std::env::var("LLM_API_KEY").unwrap_or_else(|_| "sk-or-v1-...".to_string());
    let model =
        std::env::var("LLM_MODEL").unwrap_or_else(|_| "google/gemini-3-flash-preview".to_string());
    let endpoint = std::env::var("LLM_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4000/api/v1".to_string());

    let engine = EngineBuilder::new()
        .with_key(&api_key)
        .with_model(&model)
        .with_endpoint(&endpoint)
        .build()
        .await
        .map_err(|e| vectorless::Error::Config(e.to_string()))?;

    let content = r#"# Distributed Data Processing Platform

## Introduction

This document provides a comprehensive overview of the distributed data processing platform architecture. The system is designed to handle petabyte-scale data workloads with sub-second query latency, supporting both real-time streaming and batch processing paradigms. The architecture follows a microservices-based approach with independent scaling capabilities for each component, enabling cost-effective resource utilization across varying workload patterns.


## System Architecture

The platform follows a layered architecture pattern with clear separation of concerns between ingestion, processing, storage, and serving layers. Each layer can be independently deployed, scaled, and upgraded without affecting other layers, following the principle of bounded contexts from domain-driven design. Inter-layer communication uses a combination of asynchronous message passing for data flow and synchronous gRPC calls for control plane operations.

### Ingestion Layer

The ingestion layer serves as the entry point for all data entering the platform. It supports multiple protocols including HTTP REST, gRPC, Apache Kafka, and AWS Kinesis. The layer is responsible for data validation, schema enforcement, initial transformation, and routing to downstream processing pipelines. Built on a reactive architecture using backpressure-aware operators, the ingestion layer gracefully handles burst traffic patterns without overwhelming downstream services.


### Processing Engine

The processing engine is the core computational component of the platform, responsible for transforming, enriching, aggregating, and analyzing ingested data. It supports both stream processing for real-time analytics and batch processing for historical analysis. The engine is built on a custom execution framework that optimizes query plans based on data statistics and available compute resources.

### Storage Layer

The storage layer provides a unified abstraction over multiple storage backends, each optimized for different access patterns. The hot tier uses an in-memory columnar cache for frequently accessed dimensions and recent fact data, providing microsecond-level access latency. The warm tier uses a distributed key-value store backed by NVMe SSDs for data accessed within the past 30 days. The cold tier uses object storage with Parquet file format for historical data, achieving cost efficiency at the expense of higher access latency.

Data is automatically tiered based on configurable policies that consider access frequency, data age, and query patterns. The tiering engine runs as a background service that continuously monitors access patterns and migrates data between tiers. Metadata about data placement is maintained in a distributed metadata service built on etcd, which provides consistent reads and writes with linearizable semantics.

### Query Serving Layer

The query serving layer provides the external-facing API for executing analytical queries against the processed data. It supports SQL queries via a PostgreSQL-compatible wire protocol, making it accessible to a wide range of BI tools and existing applications without requiring driver changes. The query router analyzes incoming queries and determines the optimal execution strategy, considering which storage tiers contain the relevant data and whether partial results can be served from cached aggregations.

Query results are optionally materialized in a result cache that uses a time-to-live (TTL) policy combined with lazy invalidation based on upstream data freshness markers. The cache achieves a hit rate of approximately 85% for dashboard workloads, significantly reducing the computational load on the processing engine for repetitive query patterns.

## Deployment and Operations

The platform is deployed on Kubernetes with Helm charts that encapsulate all deployment configurations, resource limits, and scaling policies. Each microservice is packaged as a container image with multi-stage builds that minimize image size and attack surface. The CI/CD pipeline uses a GitOps workflow with ArgoCD, ensuring that all changes to production are auditable, reproducible, and reversible.

Monitoring is implemented using a Prometheus and Grafana stack, with custom metrics exported by each service using a shared instrumentation library. Key performance indicators including query latency percentiles, ingestion throughput, processing lag, and error rates are tracked on operational dashboards with automated alerting through PagerDuty integration. Distributed tracing using OpenTelemetry provides end-to-end visibility into request flows across microservices, enabling rapid diagnosis of performance anomalies and error root causes.
"#;

    // Index from content string
    let result = engine
        .index(IndexContext::from_content(
            content,
            DocumentFormat::Markdown,
        ))
        .await?;

    for item in &result.items {
        println!("doc_id:  {}", item.doc_id);
        println!("name:    {}", item.name);
        println!("format:  {:?}", item.format);

        if let Some(ref metrics) = item.metrics {
            println!("time:    {}ms", metrics.total_time_ms());
            println!("nodes:   {}", metrics.nodes_processed);
            println!("tokens:  {}", metrics.total_tokens_generated);
        }
    }

    // Cleanup
    for doc in engine.list().await? {
        engine.remove(&doc.id).await?;
    }

    Ok(())
}
