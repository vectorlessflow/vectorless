// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Score Pass — computes per-node evidence quality scores.
//!
//! Analyzes leaf node content for information density, data richness,
//! and topic specificity. All metrics are pure compute, no LLM calls.

use std::collections::HashSet;
use std::time::Instant;
use tracing::{info, warn};

use vectorless_document::{EvidenceScore, EvidenceScoreMap};
use vectorless_error::Result;

use crate::passes::async_trait;
use crate::passes::{AccessPattern, CompilePass, PassResult};
use crate::pipeline::CompileContext;

/// Score Pass — builds evidence quality score map.
pub struct ScorePass;

impl ScorePass {
    /// Create a new score pass.
    pub fn new() -> Self {
        Self
    }

    /// Compute information density: unique meaningful tokens / total tokens.
    fn compute_density(content: &str) -> f64 {
        let words: Vec<&str> = content.split_whitespace().collect();
        if words.is_empty() {
            return 0.0;
        }
        let unique: HashSet<&str> = words.iter().copied().collect();
        unique.len() as f64 / words.len() as f64
    }

    /// Compute data richness: presence of numbers, tables, code, lists.
    fn compute_data_richness(content: &str) -> f64 {
        let mut score = 0.0f64;
        let len = content.len() as f64;

        if len == 0.0 {
            return 0.0;
        }

        // Numbers (digits, percentages, currencies)
        let digit_count = content.chars().filter(|c| c.is_ascii_digit()).count() as f64;
        if digit_count > 0.0 {
            score += 0.3 * (digit_count / len).min(0.1) / 0.1;
        }

        // Table markers (|, tabs, CSV patterns)
        let pipe_count = content.matches('|').count() as f64;
        let tab_count = content.matches('\t').count() as f64;
        if pipe_count > 3.0 || tab_count > 3.0 {
            score += 0.3;
        }

        // Code blocks (``` or indented lines)
        if content.contains("```") || content.contains("    ") {
            score += 0.2;
        }

        // Lists (-, *, numbered)
        let list_markers = content.lines().filter(|l| {
            let trimmed = l.trim();
            trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || trimmed.starts_with("+ ")
                || (trimmed.len() > 2 && trimmed.as_bytes().first().map(|b| b.is_ascii_digit()).unwrap_or(false) && trimmed.contains('.'))
        }).count();
        if list_markers > 0 {
            score += 0.2 * (list_markers as f64).min(5.0) / 5.0;
        }

        score.min(1.0)
    }

    /// Compute specificity: how focused the content is (vs generic filler).
    fn compute_specificity(content: &str) -> f64 {
        let words: Vec<&str> = content.split_whitespace().collect();
        if words.is_empty() {
            return 0.0;
        }

        // Generic filler words that indicate low specificity
        let filler = [
            "the", "is", "a", "an", "and", "or", "but", "in", "on", "at",
            "to", "for", "of", "with", "this", "that", "it", "from", "by",
            "was", "were", "be", "have", "has", "had", "are", "will", "would",
        ];
        let filler_set: HashSet<&str> = filler.iter().copied().collect();

        let filler_count = words.iter().filter(|w| filler_set.contains(w.to_lowercase().as_str())).count() as f64;
        let total = words.len() as f64;

        // Higher ratio of non-filler = higher specificity
        1.0 - (filler_count / total).min(0.8)
    }
}

impl Default for ScorePass {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CompilePass for ScorePass {
    fn name(&self) -> &'static str {
        "score"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["enrich"]
    }

    fn is_optional(&self) -> bool {
        true
    }

    fn access_pattern(&self) -> AccessPattern {
        AccessPattern {
            reads_tree: true,
            writes_evidence_scores: true,
            ..Default::default()
        }
    }

    async fn execute(&mut self, ctx: &mut CompileContext) -> Result<PassResult> {
        let start = Instant::now();

        let tree = match ctx.tree.as_ref() {
            Some(t) => t,
            None => {
                warn!("[score] No tree, cannot compute scores");
                return Ok(PassResult::failure("score", "Tree not built"));
            }
        };

        let leaves = tree.leaves();
        let mut score_map = EvidenceScoreMap::new();

        info!("[score] Computing evidence scores for {} leaf nodes", leaves.len());

        for &node_id in &leaves {
            let node = match tree.get(node_id) {
                Some(n) => n,
                None => continue,
            };

            let content = &node.content;
            if content.is_empty() {
                continue;
            }

            let density = Self::compute_density(content);
            let data_richness = Self::compute_data_richness(content);
            let specificity = Self::compute_specificity(content);

            score_map.insert(node_id, EvidenceScore {
                density,
                data_richness,
                specificity,
            });
        }

        let scored_count = score_map.len();
        let avg_density = if scored_count > 0 {
            score_map.scores().values().map(|s| s.density).sum::<f64>() / scored_count as f64
        } else {
            0.0
        };

        let duration = start.elapsed().as_millis() as u64;

        info!(
            "[score] Complete: {} nodes scored (avg density: {:.2}) in {}ms",
            scored_count, avg_density, duration,
        );

        ctx.metrics.record_score(duration, scored_count);

        ctx.evidence_scores = Some(score_map);

        let mut result = PassResult::success("score");
        result.duration_ms = duration;
        result.metadata.insert(
            "scored_nodes".to_string(),
            serde_json::json!(scored_count),
        );
        result.metadata.insert(
            "avg_density".to_string(),
            serde_json::json!(format!("{:.3}", avg_density)),
        );

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_density_unique() {
        // All unique words → density = 1.0
        assert!((ScorePass::compute_density("alpha beta gamma delta") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_density_repeated() {
        // "word" appears 3 times out of 3 → density = 1/3
        let d = ScorePass::compute_density("word word word");
        assert!((d - (1.0 / 3.0)).abs() < 1e-10);
    }

    #[test]
    fn test_compute_density_empty() {
        assert!((ScorePass::compute_density("") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_data_richness_numbers() {
        let score = ScorePass::compute_data_richness("Revenue was $4.2B in Q3 2024, up 12.5%");
        assert!(score > 0.0, "Should detect numbers");
    }

    #[test]
    fn test_compute_data_richness_table() {
        let score = ScorePass::compute_data_richness("| A | B | C |\n|---|---|---|\n| 1 | 2 | 3 |");
        assert!(score > 0.0, "Should detect table markers");
    }

    #[test]
    fn test_compute_data_richness_code() {
        let score = ScorePass::compute_data_richness("```rust\nfn main() {}\n```");
        assert!(score > 0.0, "Should detect code blocks");
    }

    #[test]
    fn test_compute_data_richness_list() {
        let score = ScorePass::compute_data_richness("- item one\n- item two\n- item three");
        assert!(score > 0.0, "Should detect lists");
    }

    #[test]
    fn test_compute_data_richness_plain() {
        let score = ScorePass::compute_data_richness("just some plain text without any structured data");
        assert!((score - 0.0).abs() < f64::EPSILON, "Plain text should have low richness");
    }

    #[test]
    fn test_compute_data_richness_empty() {
        assert!((ScorePass::compute_data_richness("") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_specificity_high() {
        // Lots of technical terms, few filler words
        let s = ScorePass::compute_specificity("HashMap NodeId DocumentTree CompileContext PipelineExecutor");
        assert!(s > 0.8, "Technical content should have high specificity");
    }

    #[test]
    fn test_compute_specificity_low() {
        // All filler words
        let s = ScorePass::compute_specificity("the is a an and or but in on at to for of with this that");
        assert!(s < 0.3, "Filler content should have low specificity");
    }

    #[test]
    fn test_compute_specificity_empty() {
        assert!((ScorePass::compute_specificity("") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_evidence_score_composite() {
        let score = EvidenceScore {
            density: 1.0,
            data_richness: 1.0,
            specificity: 1.0,
        };
        assert!((score.composite() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_stage_config() {
        let pass = ScorePass::new();
        assert_eq!(pass.name(), "score");
        assert!(pass.is_optional());
        assert_eq!(pass.depends_on(), vec!["enrich"]);

        let ap = pass.access_pattern();
        assert!(ap.reads_tree);
        assert!(ap.writes_evidence_scores);
        assert!(!ap.writes_tree);
    }

    #[tokio::test]
    async fn test_execute_end_to_end() {
        let mut tree = vectorless_document::DocumentTree::new("Root", "");
        let root = tree.root();
        tree.add_child(root, "Section 1", "Revenue was $4.2B in Q3 2024");
        tree.add_child(root, "Section 2", "HashMap implementation details");

        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = Some(tree);

        let mut pass = ScorePass::new();
        let result = pass.execute(&mut ctx).await;

        assert!(result.is_ok());
        assert!(result.unwrap().success);

        let scores = ctx.evidence_scores.unwrap();
        assert_eq!(scores.len(), 2);

        // All scored nodes should have positive composite
        for (_, composite) in scores.ranked_nodes() {
            assert!(composite > 0.0);
        }
    }

    #[tokio::test]
    async fn test_execute_no_tree() {
        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = None;

        let mut pass = ScorePass::new();
        let result = pass.execute(&mut ctx).await.unwrap();
        assert!(!result.success);
        assert!(ctx.evidence_scores.is_none());
    }

    #[tokio::test]
    async fn test_execute_empty_content() {
        let mut tree = vectorless_document::DocumentTree::new("Root", "");
        let root = tree.root();
        tree.add_child(root, "Empty Section", "");

        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = Some(tree);

        let mut pass = ScorePass::new();
        let result = pass.execute(&mut ctx).await;

        assert!(result.is_ok());
        assert!(result.unwrap().success);

        let scores = ctx.evidence_scores.unwrap();
        assert_eq!(scores.len(), 0); // Empty content nodes are skipped
    }
}
