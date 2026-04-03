// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index pipeline stages.

mod parse;
mod build;
mod enhance;
mod enrich;
mod optimize;
mod persist;

pub use parse::ParseStage;
pub use build::BuildStage;
pub use enhance::EnhanceStage;
pub use enrich::EnrichStage;
pub use optimize::OptimizeStage;
pub use persist::PersistStage;

pub use async_trait::async_trait;
use crate::domain::Result;
use super::pipeline::{IndexContext, StageResult};

/// Index pipeline stage.
#[async_trait]
pub trait IndexStage: Send + Sync {
    /// Stage name.
    fn name(&self) -> &str;

    /// Execute the stage.
    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult>;

    /// Whether this stage is optional (can be skipped on failure).
    fn is_optional(&self) -> bool {
        false
    }
}
