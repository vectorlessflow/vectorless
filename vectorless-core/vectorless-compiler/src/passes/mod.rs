// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Compiler passes — each pass is a discrete step in the document compilation pipeline.
//!
//! Passes are organized into four phases:
//! - **Frontend** — Parse document into AST (`parse`, `build`)
//! - **Analysis** — Semantic validation and LLM enhancement (`validate`, `enhance`)
//! - **Transform** — IR-level tree restructuring (`split`, `enrich`)
//! - **Backend** — Index generation, verification, and optimization

pub mod frontend;
pub mod analysis;
pub mod transform;
pub mod backend;

// Re-export all passes from submodules
pub use frontend::{ParsePass, BuildPass};
pub use analysis::{ValidatePass, EnhancePass};
pub use transform::{SplitPass, EnrichPass};
pub use backend::{ReasoningPass, ConceptPass, NavigationPass, VerifyPass, OptimizePass};

use super::pipeline::{FailurePolicy, CompileContext, PassResult};
pub use async_trait::async_trait;
use vectorless_error::Result;

/// Declares which context fields a pass reads/writes.
/// Used by the orchestrator to determine safe parallel execution.
#[derive(Debug, Clone, Default)]
pub struct AccessPattern {
    /// Whether this pass reads the tree.
    pub reads_tree: bool,
    /// Whether this pass mutates the tree (summaries, structure, etc.).
    pub writes_tree: bool,
    /// Whether this pass writes to `reasoning_index`.
    pub writes_reasoning_index: bool,
    /// Whether this pass writes to `navigation_index`.
    pub writes_navigation_index: bool,
    /// Whether this pass writes to `description`.
    pub writes_description: bool,
    /// Whether this pass writes to `concepts`.
    pub writes_concepts: bool,
}

/// Compiler pass trait.
///
/// Each pass represents a discrete step in the document compilation pipeline.
/// Passes are executed in dependency order by the [`PipelineOrchestrator`].
#[async_trait]
pub trait CompilePass: Send + Sync {
    /// Pass name (must be unique within pipeline).
    fn name(&self) -> &str;

    /// Execute the pass.
    async fn execute(&mut self, ctx: &mut CompileContext) -> Result<PassResult>;

    /// Whether this pass is optional (can be skipped on failure).
    fn is_optional(&self) -> bool {
        false
    }

    /// Names of passes this pass depends on.
    fn depends_on(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// Failure policy for this pass.
    fn failure_policy(&self) -> FailurePolicy {
        if self.is_optional() {
            FailurePolicy::skip()
        } else {
            FailurePolicy::fail()
        }
    }

    /// Declare which context fields this pass accesses.
    fn access_pattern(&self) -> AccessPattern {
        AccessPattern::default()
    }
}
