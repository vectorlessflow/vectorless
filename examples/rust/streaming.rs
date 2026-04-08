// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Streaming retrieval example.
//!
//! This example demonstrates how to use streaming retrieval
//! to get results incrementally as they are found.
//!
//! # What you'll learn:
//! - How to use `query_stream()` for progressive results
//! - How to handle RetrieveEvent types
//! - How to display results as they arrive
//! - How to cancel long-running queries
//!
//! # RetrieveEvent types:
//! - `Started`: Query began, shows planned strategy
//! - `NodeVisited`: A node was visited during search
//! - `ContentFound`: Relevant content was found
//! - `Backtracking`: Search is backtracking for more data
//! - `Completed`: Query finished with final results
//! - `Error`: An error occurred
//!
//! # Use cases:
//! - Interactive Q&A with real-time feedback
//! - Long-running queries on large documents
//! - Debugging retrieval behavior
//! - Building responsive UIs
//!
//! # TODO: Implementation steps
//!
//! 1. Configure engine for streaming
//! 2. Call query_stream() instead of query()
//! 3. Process events as they arrive
//! 4. Handle completion and errors

// TODO: Implement streaming retrieval
// ```
// use vectorless::client::{Engine, RetrieveEvent};
//
// async fn streaming_query(
//     engine: &Engine,
//     doc_id: &DocumentId,
//     query: &str,
// ) {
//     let mut stream = engine.query_stream(doc_id, query).await;
//
//     while let Some(event) = stream.next().await {
//         match event {
//             RetrieveEvent::Started { strategy } => {
//                 println!("Starting search with strategy: {:?}", strategy);
//             }
//             RetrieveEvent::ContentFound { node_id, preview } => {
//                 println!("Found: {} - {}", node_id, preview);
//             }
//             RetrieveEvent::Completed { response } => {
//                 println!("Done! Confidence: {}", response.confidence);
//             }
//             _ => {}
//         }
//     }
// }
// ```

fn main() {
    // TODO: Show streaming query usage
    //
    // streaming_query(&engine, &doc_id, "What is the architecture?").await;

    println!("TODO: Implement streaming example");
}
