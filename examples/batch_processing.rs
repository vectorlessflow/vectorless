// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Batch document processing example.
//!
//! This example demonstrates how to efficiently process
//! multiple documents in batch mode.
//!
//! # What you'll learn:
//! - How to index multiple documents concurrently
//! - How to batch queries for better throughput
//! - How to manage resources (memory, LLM calls) during batch processing
//! - How to track progress and handle failures
//!
//! # Use cases:
//! - Indexing a documentation site with hundreds of pages
//! - Processing a corpus of research papers
//! - Building a knowledge base from multiple sources
//!
//! # Performance considerations:
//! - Control concurrency with `max_concurrent_indexing`
//! - Use rate limiting to avoid LLM API throttling
//! - Monitor memory usage with large document sets
//!
//! # TODO: Implementation steps
//!
//! 1. Load list of documents to process
//! 2. Configure batch processing parameters
//! 3. Process documents with controlled concurrency
//! 4. Track progress and handle errors
//! 5. Generate processing report

// TODO: Implement batch processing
// ```
// use std::path::PathBuf;
// use futures::stream::{self, StreamExt};
// use vectorless::client::{Engine, EngineBuilder};
//
// async fn batch_index(
//     engine: &Engine,
//     documents: Vec<PathBuf>,
//     concurrency: usize,
// ) -> Vec<Result<DocumentId, Error>> {
//     stream::iter(documents)
//         .map(|path| async move { engine.index(&path).await })
//         .buffer_unordered(concurrency)
//         .collect()
//         .await
// }
// ```

fn main() {
    // TODO: Show batch indexing and querying
    //
    // let documents = find_all_markdown_files("./docs");
    // let results = batch_index(&engine, documents, 5).await;
    //
    // // Process results, report failures, etc.

    println!("TODO: Implement batch_processing example");
}
