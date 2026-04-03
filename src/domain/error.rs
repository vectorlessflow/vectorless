// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Error types for the vectorless library.

use thiserror::Error;

/// The main error type for vectorless operations.
#[derive(Debug, Error)]
pub enum Error {
    /// An error occurred while parsing a document.
    #[error("Document parsing error: {0}")]
    Parse(String),

    /// An error occurred while building the index.
    #[error("Index building error: {0}")]
    IndexBuild(String),

    /// An error occurred during retrieval.
    #[error("Retrieval error: {0}")]
    Retrieval(String),

    /// An error occurred during summarization.
    #[error("Summarization error: {0}")]
    Summarization(String),

    /// An error occurred during LLM call.
    #[error("LLM error: {0}")]
    Llm(String),

    /// An error occurred during I/O operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// An error occurred during serialization/deserialization.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// The requested node was not found.
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// The requested document was not found.
    #[error("Document not found: {0}")]
    DocumentNotFound(String),

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    Config(String),

    /// A generic error with a message.
    #[error("{0}")]
    Other(String),
}

/// A specialized result type for vectorless operations.
pub type Result<T> = std::result::Result<T, Error>;
