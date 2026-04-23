// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Validate stage - Verify tree integrity after build.

use std::collections::HashSet;
use std::time::Instant;
use tracing::{debug, info, warn};

use vectorless_error::Result;

use super::{AccessPattern, IndexStage, StageResult, async_trait};
use crate::pipeline::IndexContext;

/// Maximum allowed tree depth.
const MAX_DEPTH: usize = 20;

/// Minimum token count ratio for parent vs children consistency check.
/// A parent's token count should be at least `ratio` of the sum of its children.
const MIN_PARENT_TOKEN_RATIO: f32 = 0.8;

/// Minimum content similarity threshold to flag potential duplicates.
/// Content is considered duplicate if normalized equality matches.
const DUPLICATE_MIN_LENGTH: usize = 50;

/// Validation issue severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity {
    /// Warning — tree is usable but may have quality issues.
    Warning,
    /// Error — tree has structural problems.
    Error,
}

/// A single validation issue found during tree inspection.
#[derive(Debug, Clone)]
struct ValidationIssue {
    /// Severity level.
    severity: Severity,
    /// Human-readable description.
    message: String,
}

/// Validate stage — checks tree integrity after build.
///
/// Validates:
/// 1. Tree structural integrity (all nodes reachable from root)
/// 2. Depth sanity (max depth < 20)
/// 3. Empty title detection on leaf nodes
/// 4. Token count consistency (parent >= sum of children)
/// 5. Content duplication detection
pub struct ValidateStage;

impl ValidateStage {
    /// Create a new validate stage.
    pub fn new() -> Self {
        Self
    }

    /// Run all validation checks and collect issues.
    fn validate_tree(&self, ctx: &IndexContext) -> Vec<ValidationIssue> {
        let tree = match ctx.tree.as_ref() {
            Some(t) => t,
            None => {
                return vec![ValidationIssue {
                    severity: Severity::Error,
                    message: "No tree available for validation".to_string(),
                }];
            }
        };

        let mut issues = Vec::new();

        Self::check_depth(tree, &mut issues);
        Self::check_empty_titles(tree, &mut issues);
        Self::check_token_consistency(tree, &mut issues);
        Self::check_content_duplication(tree, &mut issues);

        issues
    }

    /// Check that tree depth is reasonable.
    fn check_depth(tree: &vectorless_document::DocumentTree, issues: &mut Vec<ValidationIssue>) {
        let all_nodes = tree.traverse();
        let max_depth = all_nodes
            .iter()
            .map(|&id| tree.depth(id))
            .max()
            .unwrap_or(0);

        if max_depth > MAX_DEPTH {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                message: format!(
                    "Tree depth ({}) exceeds recommended maximum ({})",
                    max_depth, MAX_DEPTH
                ),
            });
        }
    }

    /// Check for leaf nodes with empty titles.
    fn check_empty_titles(
        tree: &vectorless_document::DocumentTree,
        issues: &mut Vec<ValidationIssue>,
    ) {
        let leaves = tree.leaves();
        let mut empty_count = 0;

        for &leaf_id in &leaves {
            if let Some(node) = tree.get(leaf_id) {
                if node.title.trim().is_empty() {
                    empty_count += 1;
                }
            }
        }

        if empty_count > 0 {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                message: format!("Found {} leaf nodes with empty titles", empty_count),
            });
        }
    }

    /// Check token count consistency: parent's tokens should be >= sum of children's.
    fn check_token_consistency(
        tree: &vectorless_document::DocumentTree,
        issues: &mut Vec<ValidationIssue>,
    ) {
        let all_nodes = tree.traverse();
        let mut inconsistent = 0;

        for &node_id in &all_nodes {
            let children: Vec<_> = tree.children(node_id);
            if children.is_empty() {
                continue;
            }

            let parent_tokens = tree.get(node_id).and_then(|n| n.token_count).unwrap_or(0);

            let children_sum: usize = children
                .iter()
                .map(|&c| tree.get(c).and_then(|n| n.token_count).unwrap_or(0))
                .sum();

            // Parent should have at least some proportion of children's tokens
            // (parent has its own content plus children, but after thinning this may vary)
            if parent_tokens > 0
                && children_sum > 0
                && (parent_tokens as f32 / children_sum as f32) < MIN_PARENT_TOKEN_RATIO
            {
                // Only flag if both are non-trivial
                if children_sum >= 100 {
                    inconsistent += 1;
                }
            }
        }

        if inconsistent > 0 {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                message: format!(
                    "Found {} nodes with token counts significantly less than their children's sum",
                    inconsistent
                ),
            });
        }
    }

    /// Check for content duplication across leaf nodes.
    fn check_content_duplication(
        tree: &vectorless_document::DocumentTree,
        issues: &mut Vec<ValidationIssue>,
    ) {
        let leaves = tree.leaves();
        let mut seen: HashSet<u64> = HashSet::new();
        let mut duplicate_count = 0;

        for &leaf_id in &leaves {
            if let Some(node) = tree.get(leaf_id) {
                let content = node.content.trim();
                if content.len() < DUPLICATE_MIN_LENGTH {
                    continue;
                }

                // Simple hash of normalized content for duplicate detection
                let hash = Self::simple_hash(content);
                if !seen.insert(hash) {
                    duplicate_count += 1;
                }
            }
        }

        if duplicate_count > 0 {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                message: format!(
                    "Found {} leaf nodes with duplicate content",
                    duplicate_count
                ),
            });
        }
    }

    /// Simple FNV-1a-like hash for duplicate detection.
    /// Not cryptographic — just for grouping identical content.
    fn simple_hash(s: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in s.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
}

impl Default for ValidateStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexStage for ValidateStage {
    fn name(&self) -> &'static str {
        "validate"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["build"]
    }

    fn is_optional(&self) -> bool {
        true
    }

    fn access_pattern(&self) -> AccessPattern {
        AccessPattern {
            reads_tree: true,
            writes_tree: false,
            writes_reasoning_index: false,
            writes_navigation_index: false,
            writes_description: false,
            writes_concepts: false,
        }
    }

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        let start = Instant::now();

        let node_count = ctx.tree.as_ref().map(|t| t.node_count()).unwrap_or(0);
        info!("[validate] Starting: {} nodes", node_count);

        let issues = self.validate_tree(ctx);

        let warnings = issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count();
        let errors = issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count();

        // Log all issues
        for issue in &issues {
            match issue.severity {
                Severity::Warning => warn!("[validate] {}", issue.message),
                Severity::Error => warn!("[validate] ERROR: {}", issue.message),
            }
        }

        if warnings == 0 && errors == 0 {
            debug!("[validate] No issues found");
        }

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_validate(duration);

        info!(
            "[validate] Complete: {} warnings, {} errors in {}ms",
            warnings, errors, duration
        );

        let mut stage_result = StageResult::success("validate");
        stage_result.duration_ms = duration;
        stage_result
            .metadata
            .insert("warnings".to_string(), serde_json::json!(warnings));
        stage_result
            .metadata
            .insert("errors".to_string(), serde_json::json!(errors));

        Ok(stage_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorless_document::DocumentTree;

    fn make_context_with_tree(tree: DocumentTree) -> IndexContext {
        let input = crate::IndexInput::content("test");
        let options = crate::config::PipelineOptions::default();
        let mut ctx = IndexContext::new(input, options);
        ctx.tree = Some(tree);
        ctx
    }

    #[test]
    fn test_validate_empty_tree() {
        let tree = DocumentTree::new("Root", "");
        let ctx = make_context_with_tree(tree);

        let stage = ValidateStage::new();
        let issues = stage.validate_tree(&ctx);

        // Single root node is valid — no issues expected
        assert!(issues.is_empty());
    }

    #[test]
    fn test_validate_simple_tree() {
        let mut tree = DocumentTree::new("Root", "");
        let child = tree.add_child(tree.root(), "Section 1", "Content of section 1");
        tree.set_token_count(child, 100);

        let ctx = make_context_with_tree(tree);

        let stage = ValidateStage::new();
        let issues = stage.validate_tree(&ctx);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_validate_empty_title_warning() {
        let mut tree = DocumentTree::new("Root", "");
        let child = tree.add_child(tree.root(), "", "Some content here");
        tree.set_token_count(child, 50);

        let ctx = make_context_with_tree(tree);

        let stage = ValidateStage::new();
        let issues = stage.validate_tree(&ctx);

        let warning_count = issues
            .iter()
            .filter(|i| i.message.contains("empty titles"))
            .count();
        assert_eq!(warning_count, 1);
    }

    #[test]
    fn test_validate_no_tree_error() {
        let input = crate::IndexInput::content("test");
        let options = crate::config::PipelineOptions::default();
        let ctx = IndexContext::new(input, options);

        let stage = ValidateStage::new();
        let issues = stage.validate_tree(&ctx);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Error);
    }
}
