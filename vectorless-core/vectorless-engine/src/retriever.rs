// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document retrieval client — STUB.
//!
//! The strategy layer (agent, orchestrator, worker) has been migrated to Python.
//! This module is a stub that returns an error for any query attempt.
//! All retrieval now goes through the Python Engine.ask() path.

use vectorless_error::{Error, Result};

/// Document retrieval client (stub).
///
/// All retrieval is now handled by the Python strategy layer.
#[allow(dead_code)]
pub(crate) struct RetrieverClient;

impl RetrieverClient {
    /// Not available — retrieval is handled by Python.
    pub async fn query(&self, _question: &str) -> Result<()> {
        todo!(
            "Document retrieval is now handled by the Python strategy layer. This method should not be called."
        )
    }
}
