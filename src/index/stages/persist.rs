// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Persist stage - Save indexed document to storage.

use super::async_trait;
use std::time::Instant;
use tracing::info;

use crate::domain::Result;
use crate::storage::{PersistedDocument, DocumentMeta as StorageMeta, Workspace};

use super::{IndexStage, StageResult};
use crate::index::pipeline::IndexContext;

/// Persist stage - saves indexed document to storage.
pub struct PersistStage {
    /// Optional workspace for persistence.
    workspace: Option<Workspace>,
}

impl PersistStage {
    /// Create a new persist stage without workspace (in-memory only).
    pub fn new() -> Self {
        Self { workspace: None }
    }

    /// Create with workspace.
    pub fn with_workspace(workspace: Workspace) -> Self {
        Self {
            workspace: Some(workspace),
        }
    }

    /// Save document to workspace.
    fn save_to_workspace(&mut self, ctx: &IndexContext) -> Result<()> {
        let workspace = self.workspace.as_mut().ok_or_else(|| {
            crate::domain::Error::Config("No workspace configured".to_string())
        })?;

        let tree = ctx.tree.as_ref().ok_or_else(|| {
            crate::domain::Error::IndexBuild("Tree not built".to_string())
        })?;

        // Create metadata
        let meta = StorageMeta::new(
            &ctx.doc_id,
            &ctx.name,
            ctx.format.extension(),
        )
        .with_source_path(ctx.source_path.clone().unwrap_or_default())
        .with_description(ctx.description.clone().unwrap_or_default());

        let doc = PersistedDocument::new(meta, tree.clone());

        // Add pages if available (for PDFs)
        // Note: pages would need to be stored in context during parse stage

        workspace.add(&doc)?;
        info!("Saved document {} to workspace", ctx.doc_id);

        Ok(())
    }
}

impl Default for PersistStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexStage for PersistStage {
    fn name(&self) -> &str {
        "persist"
    }

    fn is_optional(&self) -> bool {
        true
    }

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        let start = Instant::now();

        // Only persist if workspace is configured
        if self.workspace.is_some() {
            self.save_to_workspace(ctx)?;
        } else {
            info!("No workspace configured, skipping persistence");
        }

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_persist(duration);

        info!("Persist stage completed in {}ms", duration);

        let mut stage_result = StageResult::success("persist");
        stage_result.duration_ms = duration;
        stage_result.metadata.insert(
            "persisted".to_string(),
            serde_json::json!(self.workspace.is_some()),
        );

        Ok(stage_result)
    }
}
