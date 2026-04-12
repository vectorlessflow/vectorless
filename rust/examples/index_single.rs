// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Single document indexing example — index one document from content.
//!
//! ```bash
//! cargo run --example index_single
//! ```

use vectorless::{DocumentFormat, EngineBuilder, IndexContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    let engine = EngineBuilder::new()
        .with_workspace("./workspace_single_example")
        .with_key("sk-or-v1-...")
        .with_model("google/gemini-3-flash-preview")
        .with_endpoint("http://localhost:4000/api/v1")
        .build()
        .await
        .map_err(|e| vectorless::Error::Config(e.to_string()))?;

    let content = r#"# Project Overview

## Introduction

This document describes the architecture of a distributed system
designed for high-throughput data processing.

## Components

### API Gateway

Handles authentication, rate limiting, and request routing.
Supports both REST and gRPC protocols.

### Worker Pool

Processes tasks from the message queue. Each worker handles
one task at a time with configurable timeout.

## Performance

Under load testing, the system achieves 50k requests/second
with p99 latency under 200ms.

## Conclusion

The modular design allows independent scaling of each component.
"#;

    // Index from content string
    let result = engine
        .index(IndexContext::from_content(content, DocumentFormat::Markdown))
        .await?;

    for item in &result.items {
        println!("doc_id:  {}", item.doc_id);
        println!("name:    {}", item.name);
        println!("format:  {:?}", item.format);

        if let Some(metrics) = &item.metrics {
            println!("  metrics:");
            println!("    total time:  {}ms", metrics.total_time_ms());
            println!("    parse:       {}ms", metrics.parse_time_ms);
            println!("    build:       {}ms", metrics.build_time_ms);
            println!("    enhance:     {}ms", metrics.enhance_time_ms);
            println!("    enrich:      {}ms", metrics.enrich_time_ms);
            println!("    optimize:    {}ms", metrics.optimize_time_ms);
            println!("    reasoning:   {}ms", metrics.reasoning_index_time_ms);
            println!("    nodes:       {}", metrics.nodes_processed);
            println!("    summaries:   {}", metrics.summaries_generated);
            println!("    llm calls:   {}", metrics.llm_calls);
            println!("    tokens:      {}", metrics.total_tokens_generated);
            println!("    topics:      {}", metrics.topics_indexed);
            println!("    keywords:    {}", metrics.keywords_indexed);
        }
    }

    // Cleanup
    for doc in engine.list().await? {
        engine.remove(&doc.id).await?;
    }

    Ok(())
}
