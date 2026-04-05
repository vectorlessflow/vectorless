// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Batch document processing example.
//!
//! This example demonstrates how to efficiently process
//! multiple documents in batch mode using sessions.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example batch_processing
//! ```

use vectorless::client::{EngineBuilder, IndexContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Batch Document Processing Example ===\n");

    // 1. Create engine and session
    println!("Step 1: Setting up...");
    let engine = EngineBuilder::new()
        .with_workspace("./workspace_batch_example")
        .build()
        .map_err(|e| vectorless::Error::Config(e.to_string()))?;

    let session = engine.session();
    println!("  ✓ Session created: {}\n", session.id());

    // 2. Create sample documents
    println!("Step 2: Creating sample documents...");
    let temp_dir = tempfile::tempdir()?;

    let documents = vec![
        (
            "intro.md",
            r#"# Introduction

Welcome to the vectorless library. This is a document intelligence engine.

## Features

- Tree-based navigation
- Multi-format support
- Session management
"#,
        ),
        (
            "api.md",
            r#"# API Reference

## Engine

The main client for document operations.

### Methods

- `index(path)`: Index a document
- `query(question)`: Query indexed content

## Session

Multi-document operations with caching.

### Methods

- `index(path)`: Index into session
- `query_all(question)`: Query across all documents
"#,
        ),
        (
            "guide.md",
            r#"# User Guide

## Getting Started

First, create a client with workspace configuration.

## Best Practices

- Use sessions for multi-document operations
- Enable caching for better performance
- Monitor events for debugging
"#,
        ),
        (
            "advanced.md",
            r#"# Advanced Topics

## Performance Tuning

Configure retrieval parameters for optimal performance.

### Parameters

- `top_k`: Number of results
- `max_tokens`: Token budget

## Custom Pilots

Implement custom navigation logic.
"#,
        ),
        (
            "reference.md",
            r#"# Reference

## Configuration

All configuration is done via TOML files.

### Example

```toml
[retrieval]
top_k = 5
max_tokens = 4000
```
"#,
        ),
        (
            "examples.md",
            r#"# Examples

## Basic Usage

Simple indexing and querying example.

## Batch Processing

Process multiple documents concurrently.

## Session Usage

Multi-document operations with caching.
"#,
        ),
        (
            "faq.md",
            r#"# FAQ

## Common Questions

**Q: How do I index a document?**
A: Use `engine.index(path)` method.

**Q: How to query?**
A: Use `engine.query(doc_id, question)` method.

**Q: What formats are supported?**
A: Markdown, PDF, DOCX, HTML.
"#,
        ),
        (
            "changelog.md",
            r#"# Changelog

## Version 0.1.0

- Initial release
- Basic indexing support
- Simple retrieval

## Version 0.2.0

- Session support
- Event system
- Content aggregator
"#,
        ),
        (
            "contributing.md",
            r#"# Contributing

## How to Contribute

We welcome contributions! Please follow these steps:

1. Fork the repository
2. Create a feature branch
3. Submit a pull request

## Code Style

- Run `cargo fmt`
- Run `cargo clippy`
- Add tests
"#,
        ),
        (
            "license.md",
            r#"# License

Apache License, Version 2.0

Copyright 2026 vectorless developers
"#,
        ),
        (
            "architecture.md",
            r#"# Architecture

## Overview

Vectorless uses a tree-based architecture.

## Components

- Parser: Document parsing
- Indexer: Tree building
- Retriever: Content search
- Storage: Persistence
"#,
        ),
        (
            "security.md",
            r#"# Security

## Security Considerations

- API keys are stored securely
- No sensitive data in logs
- Input validation

## Best Practices

- Use environment variables
- Rotate keys periodically
"#,
        ),
        (
            "performance.md",
            r#"# Performance

## Optimization Tips

- Use caching effectively
- Configure appropriate batch sizes
- Monitor memory usage

## Benchmarks

Run `cargo bench` for performance metrics.
"#,
        ),
        (
            "testing.md",
            r#"# Testing

## Running Tests

```bash
cargo test
```

## Test Coverage

- Unit tests
- Integration tests
- Example tests
"#,
        ),
        (
            "deployment.md",
            r#"# Deployment

## Production Setup

- Configure workspace directory
- Set up logging
- Monitor performance

## Configuration

Use TOML configuration files.
"#,
        ),
        (
            "troubleshooting.md",
            r#"# Troubleshooting

## Common Issues

### Indexing Fails

Check file format and permissions.

### Query Returns Empty

Ensure document is indexed.

### Performance Issues

Reduce batch size or enable caching.
"#,
        ),
        (
            "integrations.md",
            r#"# Integrations

## LLM Providers

- OpenAI
- Anthropic
- Local models

## Storage Backends

- File system (default)
- S3 (planned)
"#,
        ),
        (
            "migrations.md",
            r#"# Migrations

## Version Migrations

### 0.1.x to 0.2.x

- Update configuration format
- Re-index documents
"#,
        ),
        (
            "roadmap.md",
            r#"# Roadmap

## Future Plans

### Short Term

- Streaming support
- More formats

### Long Term

- Distributed indexing
- Real-time updates
"#,
        ),
        (
            "credits.md",
            r#"# Credits

## Contributors

Thanks to all contributors!

## Libraries

Built with Rust and many open-source libraries.
"#,
        ),
        (
            "index.md",
            r#"# Index

## Quick Links

- [Introduction](intro.md)
- [API Reference](api.md)
- [User Guide](guide.md)

## Search

Use the search functionality to find specific content.
"#,
        ),
        (
            "search.md",
            r#"# Search

## Search Functionality

### Basic Search

```rust
let results = engine.query(&doc_id, "search term").await?;
```

### Advanced Search

Use sessions for cross-document search.
"#,
        ),
        (
            "export.md",
            r#"# Export

## Exporting Data

### JSON Export

```rust
let json = tree.to_structure_json();
```

### Custom Formats

Implement custom exporters as needed.
"#,
        ),
        (
            "import.md",
            r#"# Import

## Importing Data

### From Files

```rust
let doc_id = engine.index("./document.md").await?;
```

### From Memory

Use the content directly with parsers.
"#,
        ),
        (
            "validation.md",
            r#"# Validation

## Input Validation

### Document Paths

Must exist and be readable.

### Configuration

Validated on load with helpful errors.

### Queries

Sanitized before processing.
"#,
        ),
        (
            "formatting.md",
            r#"# Formatting

## Content Formatting

### Markdown

Standard CommonMark with extensions.

### Code Blocks

Syntax highlighting support.

### Tables

Basic table parsing.
"#,
        ),
        (
            "localization.md",
            r#"# Localization

## Internationalization

Currently English-only.

## Future Support

Planned i18n support for:
- Error messages
- UI strings
- Documentation
"#,
        ),
        (
            "accessibility.md",
            r#"# Accessibility

## Accessibility

### Documentation

Clear and comprehensive docs.

### API Design

Consistent and intuitive naming.

### Error Messages

Helpful and actionable.
"#,
        ),
        (
            "glossary.md",
            r#"# Glossary

## Terms

- **Document Tree**: Hierarchical structure
- **Session**: Multi-document context
- **Workspace**: Document storage
- **Retrieval**: Content search
"#,
        ),
        (
            "appendix.md",
            r#"# Appendix

## Additional Resources

- [GitHub Repository](https://github.com)
- [Documentation Site](https://docs.vectorless.dev)
- [Community Discord](https://discord.gg)
"#,
        ),
        (
            "summary.md",
            r#"# Summary

## Overview

This documentation covers all aspects of vectorless.

## Next Steps

- Try the examples
- Join the community
- Contribute!
"#,
        ),
        (
            "conclusion.md",
            r#"# Conclusion

## Thank You

Thanks for using vectorless!

## Feedback

We'd love to hear from you. Open an issue on GitHub.
"#,
        ),
        (
            "revision.md",
            r#"# Revision History

## Document Versions

| Version | Date       | Changes                    |
|---------|------------|---------------------------|
| 1.0     | 2026-01-01 | Initial version           |
| 1.1     | 2026-02-01 | Session support           |
"#,
        ),
        (
            "feedback.md",
            r#"# Feedback

## Providing Feedback

We value your input!

### Channels

- GitHub Issues
- Discord Community
- Email Support

### What to Share

- Bug reports
- Feature requests
- Documentation improvements
"#,
        ),
        (
            "support.md",
            r#"# Support

## Getting Help

### Documentation

Start with the user guide.

### Community

Join our Discord for discussions.

### Enterprise

Contact us for enterprise support.
"#,
        ),
        (
            "updates.md",
            r#"# Updates

## Staying Updated

### Version Updates

Check the changelog for updates.

### Security Updates

Apply security patches promptly.

### Deprecations

Watch for deprecation notices.
"#,
        ),
        (
            "resources.md",
            r#"# Resources

## External Resources

### Official

- Documentation: docs.vectorless.dev
- GitHub: github.com/vectorless
- Discord: discord.gg/vectorless

### Community

- Blog posts
- Tutorial videos
- Example projects
"#,
        ),
        (
            "contact.md",
            r#"# Contact

## Contact Information

### General Inquiries

Email: hello@vectorless.dev

### Security Issues

Email: security@vectorless.dev

### Enterprise Sales

Email: enterprise@vectorless.dev
"#,
        ),
        (
            "privacy.md",
            r#"# Privacy Policy

## Data Handling

Vectorless processes documents locally.

## No Tracking

We don't track usage or content.

## API Keys

Stored securely in configuration files.
"#,
        ),
        (
            "terms.md",
            r#"# Terms of Service

## Usage Terms

By using vectorless, you agree to:

- Use responsibly
- Follow applicable laws
- Respect rate limits

## Changes

Terms may be updated. Check for revisions.
"#,
        ),
        (
            "legal.md",
            r#"# Legal

## Licensing

Apache License 2.0

## Copyright

Copyright 2026 vectorless developers

## Trademarks

Vectorless is a trademark.
"#,
        ),
        (
            "versioning.md",
            r#"# Versioning

## Semantic Versioning

We follow semver:

- MAJOR: Breaking changes
- MINOR: New features
- PATCH: Bug fixes

## Current Version

0.1.10
"#,
        ),
        (
            "compatibility.md",
            r#"# Compatibility

## Supported Versions

- Rust 1.70+
- Tokio 1.x

## Platform Support

- Linux
- macOS
- Windows

## Breaking Changes

Documented in changelog.
"#,
        ),
        (
            "installation.md",
            r#"# Installation

## Requirements

- Rust 1.70+
- Tokio runtime

## Install

```bash
cargo install vectorless
```

## Verify

```bash
vectorless --version
```
"#,
        ),
        (
            "quickstart.md",
            r#"# Quick Start

## 5-Minute Setup

1. Install vectorless
2. Create a client
3. Index a document
4. Query!

```rust
let client = Engine::builder().build()?;
let doc_id = client.index("./doc.md").await?;
let result = client.query(&doc_id, "What is this?").await?;
```
"#,
        ),
        (
            "tutorial.md",
            r#"# Tutorial

## Introduction

This tutorial covers basic usage.

## Step 1: Setup

Create a client with workspace.

## Step 2: Index

Index your first document.

## Step 3: Query

Ask questions about your document.

## Step 4: Next

Explore advanced features.
"#,
        ),
        (
            "examples_overview.md",
            r#"# Examples Overview

## Available Examples

| Example         | Description                    |
|-----------------|--------------------------------|
| basic.rs        | Basic usage                   |
| session.rs      | Multi-document operations     |
| events.rs       | Event callbacks              |
| batch.rs        | Batch processing             |

## Running Examples

```bash
cargo run --example <name>
```
"#,
        ),
        (
            "configuration.md",
            r#"# Configuration

## Configuration File

Use `config.toml` for settings:

```toml
[storage]
workspace_dir = "./workspace"

[retrieval]
top_k = 5
max_tokens = 4000
```

## Environment Variables

- `OPENAI_API_KEY`: LLM API key
"#,
        ),
        (
            "optimization.md",
            r#"# Optimization

## Performance Tips

- Use sessions for caching
- Batch document indexing
- Configure appropriate token limits

## Memory Management

Documents are cached in sessions.

## Concurrency

Use `buffer_unordered` for parallel indexing.
"#,
        ),
        (
            "errors.md",
            r#"# Error Handling

## Error Types

- `ConfigError`: Configuration issues
- `ParseError`: Document parsing failures
- `RetrievalError`: Query failures

## Handling Errors

```rust
match result {
    Ok(response) => { /* success */ },
    Err(Error::Parse(msg)) => { /* handle parse error */ },
    Err(e) => { /* other error */ },
}
```
"#,
        ),
        (
            "logging.md",
            r#"# Logging

## Log Levels

- ERROR: Serious issues
- WARN: Potential issues
- INFO: General information
- DEBUG: Detailed information
- TRACE: Very detailed

## Enabling Logs

```bash
RUST_LOG=debug cargo run
```
"#,
        ),
        (
            "metrics.md",
            r#"# Metrics

## Available Metrics

- Query count
- Cache hit rate
- Average query time

## Accessing Metrics

```rust
let stats = session.stats();
println!("Cache hit rate: {:.1}%", stats.cache_hit_rate() * 100.0);
```
"#,
        ),
        (
            "health.md",
            r#"# Health Checks

## Workspace Health

Check workspace integrity:

```rust
let docs = engine.list_documents();
println!("{} documents indexed", docs.len());
```

## Session Health

Monitor session statistics regularly.
"#,
        ),
        (
            "backup.md",
            r#"# Backup

## Backing Up

Copy the workspace directory:

```bash
cp -r ./workspace ./workspace_backup
```

## Restoration

Restore by copying back:

```bash
cp -r ./workspace_backup ./workspace
```
"#,
        ),
        (
            "recovery.md",
            r#"# Recovery

## Corrupted Documents

Remove and re-index:

```rust
engine.remove(&doc_id)?;
engine.index(&path).await?;
```

## Session Recovery

Create a new session if issues occur.
"#,
        ),
        (
            "monitoring.md",
            r#"# Monitoring

## Production Monitoring

Use events for real-time monitoring:

```rust
let events = EventEmitter::new()
    .on_query(|e| {
        // Log to monitoring system
    });
```

## Alerts

Set up alerts for error rates.
"#,
        ),
        (
            "scaling.md",
            r#"# Scaling

## Horizontal Scaling

Run multiple instances with shared storage.

## Vertical Scaling

Increase resources for single instance.

## Considerations

- Storage backend
- Cache coordination
- Rate limiting
"#,
        ),
        (
            "security_config.md",
            r#"# Security Configuration

## API Keys

Store securely:

```toml
[summary]
api_key = "${OPENAI_API_KEY}"
```

## Network Security

Use HTTPS for all API calls.

## Access Control

Implement authentication for production.
"#,
        ),
    ];

    for (name, content) in &documents {
        let path = temp_dir.path().join(name);
        std::fs::write(&path, content)?;
    }

    println!("  ✓ Created {} sample documents\n", documents.len());

    // 3. Batch indexing with progress
    println!("Step 3: Batch indexing...");
    let start = std::time::Instant::now();
    let mut doc_ids = Vec::new();

    for (name, _) in &documents {
        let path = temp_dir.path().join(name);
        match session.index(IndexContext::from_path(&path)).await {
            Ok(doc_id) => {
                doc_ids.push(doc_id);
            }
            Err(e) => {
                eprintln!("  ✗ Failed to index {}: {}", name, e);
            }
        }
    }

    let elapsed = start.elapsed();
    println!("  ✓ Indexed {} documents in {:?}", doc_ids.len(), elapsed);
    println!(
        "  - Rate: {:.1} docs/sec",
        doc_ids.len() as f64 / elapsed.as_secs_f64()
    );
    println!();

    // 4. Show session stats
    println!("Step 4: Session statistics:");
    let stats = session.stats();
    println!(
        "  - Documents in session: {}",
        session.list_documents().len()
    );
    println!("  - Queries: {}", stats.query_count.get());
    println!();

    // 5. Batch query with progress
    println!("Step 5: Batch querying...");
    let queries = vec![
        "What is vectorless?",
        "How to index?",
        "Configuration options",
        "API methods",
        "Performance tips",
        "Error handling",
        "Logging setup",
        "Security considerations",
        "Scaling options",
        "Getting help",
    ];

    let start = std::time::Instant::now();
    let mut success_count = 0;

    for query in &queries {
        match session.query_all(query).await {
            Ok(results) => {
                if !results.is_empty() {
                    success_count += 1;
                }
            }
            Err(e) => {
                eprintln!("  ✗ Query failed: {}", e);
            }
        }
    }

    let elapsed = start.elapsed();
    println!("  ✓ Completed {} queries in {:?}", queries.len(), elapsed);
    println!(
        "  - Success rate: {:.0}%",
        (success_count as f64 / queries.len() as f64) * 100.0
    );
    println!(
        "  - Rate: {:.1} queries/sec",
        queries.len() as f64 / elapsed.as_secs_f64()
    );
    println!();

    // 6. Final statistics
    println!("Step 6: Final statistics:");
    let stats = session.stats();
    println!("  - Total documents: {}", session.list_documents().len());
    println!("  - Total queries: {}", stats.query_count.get());
    println!("  - Cache hits: {}", stats.cache_hits.get());
    println!("  - Cache misses: {}", stats.cache_misses.get());
    println!("  - Cache hit rate: {:.1}%", stats.cache_hit_rate() * 100.0);
    if let Some(avg_time) = stats.avg_query_time() {
        println!("  - Avg query time: {:?}", avg_time);
    }
    println!("  - Session age: {:?}", session.age());
    println!();

    // 7. Cleanup
    println!("Step 7: Cleanup...");
    for doc_id in &doc_ids {
        engine.remove(doc_id).await?;
    }
    println!("  ✓ Removed {} documents\n", doc_ids.len());

    println!("=== Example Complete ===");
    Ok(())
}
