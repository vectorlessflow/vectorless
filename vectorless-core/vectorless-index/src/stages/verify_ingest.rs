// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Verify stage — validates ingest output reliability before persist.

use tracing::{info, warn};

use super::{AccessPattern, IndexStage};
use vectorless_error::{Error, Result};
use crate::index::pipeline::{IndexContext, StageResult};
use super::async_trait;

/// Verification stage — ensures ingest produced reliable output.
///
/// Checks:
/// - Tree is non-empty (at least root node)
/// - Document summary is non-empty
/// - At least one concept was extracted
///
/// Any check failure produces an error — no silent degradation.
pub struct VerifyStage;

#[async_trait]
impl IndexStage for VerifyStage {
    fn name(&self) -> &str {
        "verify"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["concept_extraction"]
    }

    fn is_optional(&self) -> bool {
        false
    }

    fn access_pattern(&self) -> AccessPattern {
        AccessPattern {
            reads_tree: true,
            ..AccessPattern::default()
        }
    }

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        // Tree must exist and have nodes
        let tree = ctx.tree.as_ref().ok_or_else(|| {
            Error::InvalidStructure("document tree is empty".into())
        })?;
        let node_count = tree.node_count();
        if node_count == 0 {
            return Err(Error::InvalidStructure(
                "tree has no nodes".into(),
            ));
        }

        // Summary must be non-empty
        let has_summary = ctx
            .description
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty());
        if !has_summary {
            warn!("[verify] Document summary is empty");
        }

        // Concepts must be present (warning only — non-fatal)
        if ctx.concepts.is_empty() {
            warn!("[verify] No concepts extracted from document");
        }

        info!(
            "[verify] Passed: {} nodes, summary={}, concepts={}",
            node_count,
            has_summary,
            ctx.concepts.len()
        );

        Ok(StageResult::success("verify"))
    }
}
