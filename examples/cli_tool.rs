// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! CLI tool example for vectorless.
//!
//! This example shows how to build a command-line tool
//! using vectorless for document indexing and querying.
//!
//! # What you'll learn:
//! - How to structure a CLI application
//! - How to handle subcommands (index, query, info)
//! - How to manage configuration and workspace
//! - How to provide user-friendly output
//!
//! # Example commands:
//!
//! ```bash
//! # Index a document
//! vectorless-cli index ./document.md
//!
//! # Query a document
//! vectorless-cli query <doc-id> "What is the main topic?"
//!
//! # List indexed documents
//! vectorless-cli list
//!
//! # Show document info
//! vectorless-cli info <doc-id>
//!
//! # Delete a document
//! vectorless-cli delete <doc-id>
//! ```
//!
//! # Implementation notes:
//!
//! ## Recommended crates:
//! - `clap` for argument parsing
//! - `colored` or `termcolor` for colored output
//! - `indicatif` for progress bars
//! - `serde` for configuration
//!
//! ## Configuration file:
//! ```toml
//! # ~/.vectorless/config.toml
//! [llm]
//! provider = "openai"
//! model = "gpt-4"
//!
//! [index]
//! cache_size = 100
//!
//! [retrieval]
//! max_iterations = 10
//! ```
//!
//! # TODO: Implementation steps
//!
//! 1. Define CLI structure with clap
//! 2. Implement index subcommand
//! 3. Implement query subcommand
//! 4. Implement list/info subcommands
//! 5. Add configuration management
//! 6. Add colored output and progress

// TODO: Implement CLI tool
// ```
// use clap::{Parser, Subcommand};
// use vectorless::client::{Engine, EngineBuilder};
//
// #[derive(Parser)]
// #[command(name = "vectorless-cli")]
// struct Cli {
//     #[command(subcommand)]
//     command: Commands,
// }
//
// #[derive(Subcommand)]
// enum Commands {
//     /// Index a document
//     Index {
//         /// Path to document
//         path: PathBuf,
//     },
//     /// Query an indexed document
//     Query {
//         /// Document ID
//         doc_id: String,
//         /// Query string
//         query: String,
//     },
//     /// List all indexed documents
//     List,
// }
//
// #[tokio::main]
// async fn main() -> Result<()> {
//     let cli = Cli::parse();
//     let engine = EngineBuilder::new().build()?;
//
//     match cli.command {
//         Commands::Index { path } => {
//             let doc_id = engine.index(&path).await?;
//             println!("Indexed: {}", doc_id);
//         }
//         Commands::Query { doc_id, query } => {
//             let result = engine.query(&doc_id, &query).await?;
//             println!("{}", result.content);
//         }
//         Commands::List => {
//             // List documents
//         }
//     }
//
//     Ok(())
// }
// ```

fn main() {
    // TODO: Implement full CLI tool

    println!("TODO: Implement cli_tool example");
}
