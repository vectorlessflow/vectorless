// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Concept extraction stage — extracts key concepts from topics and summaries.

use std::collections::HashMap;

use serde::Deserialize;
use tracing::{info, warn};

use vectorless_document::Concept;
use vectorless_error::Result;
use vectorless_llm::LlmClient;

use super::async_trait;
use super::{AccessPattern, IndexStage, StageResult};
use crate::pipeline::IndexContext;

/// Maximum number of top keywords to send to the LLM for concept extraction.
const MAX_TOPICS: usize = 20;

/// Maximum number of concepts to extract.
const MAX_CONCEPTS: usize = 15;

/// Concept extraction stage.
///
/// Takes the reasoning index's topic entries and tree summaries, then uses
/// a single LLM call to extract structured [`Concept`] values.
/// Falls back to basic keyword-based concepts when no LLM is available.
pub struct ConceptExtractionStage {
    llm_client: Option<LlmClient>,
}

impl ConceptExtractionStage {
    /// Create a new stage without LLM support (keyword-based fallback).
    pub fn new() -> Self {
        Self { llm_client: None }
    }

    /// Create a stage with LLM support for rich concept extraction.
    pub fn with_llm_client(client: LlmClient) -> Self {
        Self {
            llm_client: Some(client),
        }
    }
}

#[async_trait]
impl IndexStage for ConceptExtractionStage {
    fn name(&self) -> &str {
        "concept_extraction"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["reasoning_index"]
    }

    fn is_optional(&self) -> bool {
        true
    }

    fn access_pattern(&self) -> AccessPattern {
        AccessPattern {
            reads_tree: true,
            writes_concepts: true,
            ..AccessPattern::default()
        }
    }

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        let concepts = if let Some(ref client) = self.llm_client {
            extract_with_llm(ctx, client).await
        } else {
            extract_from_topics(ctx)
        };

        let count = concepts.len();
        ctx.concepts = concepts;
        info!("[concept_extraction] Extracted {} concepts", count);

        Ok(StageResult::success("concept_extraction"))
    }
}

/// Extract concepts using LLM from topics and summaries.
async fn extract_with_llm(ctx: &mut IndexContext, client: &LlmClient) -> Vec<Concept> {
    let (topics, section_titles) = gather_source_data(ctx);

    if topics.is_empty() {
        warn!("[concept_extraction] No topics available for extraction");
        return Vec::new();
    }

    let system = "You are a document analysis assistant. Extract the most important concepts \
        from the given topics and section titles. For each concept, provide:\n\
        - name: a short name (2-4 words)\n\
        - summary: a one-sentence explanation\n\
        - sections: list of section titles where this concept appears\n\n\
        Return ONLY a valid JSON array of objects. No explanation, no markdown. \
        Maximum 15 concepts, ordered by importance.";

    let user_prompt = format!(
        "Document topics (keyword: relevance weight):\n{}\n\n\
         Section titles:\n{}",
        topics
            .iter()
            .map(|(k, w)| format!("- {} (weight: {:.2})", k, w))
            .collect::<Vec<_>>()
            .join("\n"),
        section_titles.join(", "),
    );

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    struct RawConcept {
        name: String,
        summary: String,
        #[serde(default)]
        sections: Vec<String>,
    }

    match client
        .complete_json::<Vec<RawConcept>>(&system, &user_prompt)
        .await
    {
        Ok(raw) => raw
            .into_iter()
            .take(MAX_CONCEPTS)
            .map(|c| Concept {
                name: c.name,
                summary: c.summary,
                sections: c.sections,
            })
            .collect(),
        Err(e) => {
            warn!("[concept_extraction] LLM extraction failed: {}, using fallback", e);
            extract_from_topics(ctx)
        }
    }
}

/// Fallback: derive basic concepts from topic keywords.
fn extract_from_topics(ctx: &mut IndexContext) -> Vec<Concept> {
    let (topics, section_titles) = gather_source_data(ctx);

    topics
        .into_iter()
        .take(MAX_CONCEPTS)
        .map(|(name, _)| Concept {
            name: name.clone(),
            summary: String::new(),
            sections: section_titles.clone(),
        })
        .collect()
}

/// Gather top topics and section titles from the pipeline context.
fn gather_source_data(ctx: &IndexContext) -> (Vec<(String, f32)>, Vec<String>) {
    // Collect top keywords by weight
    let mut topics: Vec<(String, f32)> = Vec::new();

    if let Some(ref ri) = ctx.reasoning_index {
        let mut all: Vec<(String, f32)> = ri
            .all_topic_entries()
            .map(|(keyword, entries)| {
                let max_weight = entries.iter().map(|e| e.weight).fold(0.0_f32, f32::max);
                (keyword.clone(), max_weight)
            })
            .collect();
        all.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        all.truncate(MAX_TOPICS);
        topics = all;
    }

    // Collect section titles from the tree
    let section_titles: Vec<String> = ctx
        .tree
        .as_ref()
        .map(|tree| {
            tree.traverse()
                .iter()
                .filter_map(|&id| {
                    let node = tree.get(id)?;
                    if !node.title.is_empty() {
                        Some(node.title.clone())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    (topics, section_titles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_from_empty_topics() {
        let topics = Vec::<(String, f32)>::new();
        let titles = vec!["Section 1".to_string()];
        // Basic sanity: empty topics produce empty concepts
        let concepts: Vec<Concept> = topics
            .into_iter()
            .take(MAX_CONCEPTS)
            .map(|(name, _)| Concept {
                name,
                summary: String::new(),
                sections: titles.clone(),
            })
            .collect();
        assert!(concepts.is_empty());
    }

    #[test]
    fn test_extract_from_topics_basic() {
        let topics: Vec<(String, f32)> = vec![
            ("quantum".to_string(), 0.95),
            ("error correction".to_string(), 0.88),
            ("qubit".to_string(), 0.82),
        ];
        let titles = vec!["Research Labs".to_string()];
        let concepts: Vec<Concept> = topics
            .into_iter()
            .take(MAX_CONCEPTS)
            .map(|(name, _)| Concept {
                name,
                summary: String::new(),
                sections: titles.clone(),
            })
            .collect();
        assert_eq!(concepts.len(), 3);
        assert_eq!(concepts[0].name, "quantum");
    }
}
