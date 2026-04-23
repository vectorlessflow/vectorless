// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Evidence deduplication and quality filtering.

use std::collections::HashSet;

use vectorless_agent::Evidence;

/// Minimum characters for an evidence item to be considered meaningful.
const MIN_EVIDENCE_CHARS: usize = 50;

/// Jaccard similarity threshold for content dedup.
const SIMILARITY_THRESHOLD: f64 = 0.8;

/// Filter low-quality and duplicate evidence.
///
/// Steps:
/// 1. Drop evidence with no meaningful content (< MIN_EVIDENCE_CHARS)
/// 2. Deduplicate by source overlap (same path in same doc)
/// 3. Deduplicate by content similarity (Jaccard on token sets)
pub fn dedup(evidence: &[Evidence]) -> Vec<Evidence> {
    // Step 1: Quality filter
    let quality: Vec<&Evidence> = evidence
        .iter()
        .filter(|e| e.content.len() >= MIN_EVIDENCE_CHARS)
        .collect();

    // Step 2: Deduplicate by source overlap
    let mut seen_sources: HashSet<String> = HashSet::new();
    let source_deduped: Vec<&Evidence> = quality
        .into_iter()
        .filter(|e| {
            let doc_key = e.doc_name.as_deref().unwrap_or("_unknown");
            let key = format!("{}:{}", doc_key, e.source_path);
            seen_sources.insert(key)
        })
        .collect();

    // Step 3: Deduplicate by content similarity
    let mut deduped: Vec<Evidence> = Vec::new();
    for ev in source_deduped {
        let tokens = tokenize(&ev.content);
        let dominated = deduped
            .iter()
            .any(|existing| jaccard(&tokens, &tokenize(&existing.content)) >= SIMILARITY_THRESHOLD);
        if !dominated {
            deduped.push(ev.clone());
        }
    }

    deduped
}

/// Tokenize text into a set of lowercase words.
fn tokenize(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// Compute Jaccard similarity between two sets.
fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let intersection = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    intersection / union
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_evidence(title: &str, content: &str) -> Evidence {
        Evidence {
            source_path: format!("root/{}", title),
            node_title: title.to_string(),
            content: content.to_string(),
            doc_name: Some("doc".to_string()),
        }
    }

    #[test]
    fn test_quality_filter() {
        let evidence = vec![
            make_evidence("A", "short"),         // < 50 chars, filtered
            make_evidence("B", &"x".repeat(60)), // kept
        ];
        let result = dedup(&evidence);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].node_title, "B");
    }

    #[test]
    fn test_source_dedup() {
        let evidence = vec![
            make_evidence(
                "A",
                &"content A with enough text to pass the quality filter threshold".to_string(),
            ),
            make_evidence(
                "A",
                &"different content A but same source path that is long enough".to_string(),
            ),
        ];
        let result = dedup(&evidence);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_content_similarity_dedup() {
        let base = "This is a piece of evidence about machine learning algorithms and their applications in real world scenarios".to_string();
        let similar = "This is a piece of evidence about machine learning algorithms and their applications in real world".to_string();
        let different =
            "Completely unrelated content about quantum physics and particle accelerators at CERN"
                .to_string();
        let evidence = vec![
            make_evidence("A", &base),
            make_evidence("B", &similar), // high similarity, should be deduped
            make_evidence("C", &different), // different, kept
        ];
        let result = dedup(&evidence);
        assert!(result.len() >= 2); // at least A and C
    }

    #[test]
    fn test_empty_input() {
        let result = dedup(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_jaccard_identical() {
        let a = tokenize("hello world foo");
        let b = tokenize("hello world foo");
        assert!((jaccard(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_disjoint() {
        let a = tokenize("aaa bbb");
        let b = tokenize("ccc ddd");
        assert!((jaccard(&a, &b)).abs() < 0.001);
    }

    #[test]
    fn test_source_dedup_none_doc_name() {
        // Evidence with doc_name: None should use "_unknown" as doc key,
        // so same source_path with None doc_name still deduplicates correctly.
        let evidence = vec![
            Evidence {
                source_path: "root/section_a".to_string(),
                node_title: "A".to_string(),
                content: "content A with enough text to pass the quality filter threshold"
                    .to_string(),
                doc_name: None,
            },
            Evidence {
                source_path: "root/section_a".to_string(),
                node_title: "A2".to_string(),
                content: "different content but same source path that should be deduped"
                    .to_string(),
                doc_name: None,
            },
        ];
        let result = dedup(&evidence);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_source_dedup_mixed_doc_name() {
        // Same source_path but different doc_name should produce different dedup keys,
        // so both survive source dedup. Content must be sufficiently different too.
        let evidence = vec![
            Evidence {
                source_path: "root/section".to_string(),
                node_title: "A".to_string(),
                content: "Revenue for Q4 was twelve million dollars driven by SaaS growth in the enterprise segment".to_string(),
                doc_name: Some("doc_a".to_string()),
            },
            Evidence {
                source_path: "root/section".to_string(),
                node_title: "B".to_string(),
                content: "The encryption module uses AES-256 for data at rest and TLS 1.3 for all network communication".to_string(),
                doc_name: Some("doc_b".to_string()),
            },
        ];
        let result = dedup(&evidence);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_source_dedup_none_vs_some_doc_name() {
        // None doc_name ("_unknown") and Some doc_name produce different keys,
        // so both survive source dedup. Content must be sufficiently different too.
        let evidence = vec![
            Evidence {
                source_path: "root/section".to_string(),
                node_title: "A".to_string(),
                content: "The database uses a log-structured merge tree with write-ahead logging for durability".to_string(),
                doc_name: None,
            },
            Evidence {
                source_path: "root/section".to_string(),
                node_title: "B".to_string(),
                content: "Authentication requires Bearer tokens with automatic refresh after twenty-four hours".to_string(),
                doc_name: Some("doc_x".to_string()),
            },
        ];
        let result = dedup(&evidence);
        assert_eq!(result.len(), 2);
    }
}
