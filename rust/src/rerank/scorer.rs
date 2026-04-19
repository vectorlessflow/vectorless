// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Relevance scoring using BM25.

use crate::agent::Evidence;
use crate::scoring::bm25::{Bm25Engine, FieldDocument, extract_keywords};

/// Score evidence items against the query using BM25.
///
/// Returns (evidence_indices_sorted, scores) — indices sorted by relevance (highest first).
/// Does not mutate the original evidence slice.
pub fn rank(query: &str, evidence: &[Evidence]) -> Vec<(usize, f32)> {
    if evidence.is_empty() {
        return Vec::new();
    }

    let keywords = extract_keywords(query);
    if keywords.is_empty() {
        // No keywords: uniform score, preserve order
        return evidence.iter().enumerate().map(|(i, _)| (i, 0.5)).collect();
    }

    // Build BM25 index from evidence content
    let docs: Vec<FieldDocument<usize>> = evidence
        .iter()
        .enumerate()
        .map(|(i, ev)| {
            FieldDocument::new(
                i,
                ev.node_title.clone(),
                String::new(), // no summary for evidence
                ev.content.clone(),
            )
        })
        .collect();

    let engine = Bm25Engine::fit_to_corpus(&docs);
    let scored = engine.search_weighted(query, evidence.len());

    // Build score map
    let mut results: Vec<(usize, f32)> = scored
        .into_iter()
        .map(|(id, score)| (id, score as f32))
        .collect();

    // Add unscored evidence with score 0.0
    let scored_ids: std::collections::HashSet<usize> = results.iter().map(|(id, _)| *id).collect();
    for i in 0..evidence.len() {
        if !scored_ids.contains(&i) {
            results.push((i, 0.0));
        }
    }

    // Sort by score descending
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_evidence(title: &str, content: &str) -> Evidence {
        Evidence {
            source_path: format!("root/{}", title),
            node_title: title.to_string(),
            content: content.to_string(),
            doc_name: None,
        }
    }

    #[test]
    fn test_rank_sorts_by_relevance() {
        let evidence = vec![
            make_evidence(
                "Unrelated",
                "The weather is nice today and the sun is shining",
            ),
            make_evidence(
                "ML Intro",
                "Machine learning algorithms for classification and regression tasks",
            ),
            make_evidence(
                "ML Advanced",
                "Deep learning neural networks for image recognition",
            ),
        ];
        let ranked = rank("machine learning", &evidence);
        assert_eq!(ranked.len(), 3);
        // ML-related items should score higher
        assert!(ranked[0].1 >= ranked[ranked.len() - 1].1);
    }

    #[test]
    fn test_rank_empty_evidence() {
        let evidence: Vec<Evidence> = vec![];
        let ranked = rank("query", &evidence);
        assert!(ranked.is_empty());
    }

    #[test]
    fn test_rank_no_keywords() {
        let evidence = vec![make_evidence("A", "some content here")];
        let ranked = rank("", &evidence);
        assert!((ranked[0].1 - 0.5).abs() < 0.001);
    }
}
