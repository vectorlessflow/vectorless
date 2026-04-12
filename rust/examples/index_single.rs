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
        if let Some(m) = &item.metrics {
            println!("time:    {}ms, nodes: {}", m.total_time_ms(), m.nodes_processed);
        }
    }

    // Cleanup
    for doc in engine.list().await? {
        engine.remove(&doc.id).await?;
    }

    Ok(())
}
