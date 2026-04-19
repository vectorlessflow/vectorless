// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Orchestrator tools: ls_docs, find_cross, dispatch.

use super::ToolResult;
use crate::retrieval::agent::config::WorkspaceContext;

/// Execute `ls_docs` — list all document cards.
///
/// Returns a formatted view of all DocCards for the Orchestrator's Bird's-Eye View.
pub fn ls_docs(ctx: &WorkspaceContext) -> ToolResult {
    let cards = ctx.doc_cards();

    if cards.is_empty() {
        return ToolResult::ok("No documents with DocCards available.");
    }

    let mut output = format!("Available documents ({} total):\n\n", ctx.doc_count());

    for (idx, card) in &cards {
        output.push_str(&format!(
            "[{}] {} — {}\n",
            idx + 1,
            card.title,
            card.overview
        ));

        for sec in &card.sections {
            output.push_str(&format!(
                "    → {} ({} leaves)\n",
                sec.title, sec.leaf_count
            ));
        }

        if !card.question_hints.is_empty() {
            output.push_str(&format!(
                "    Can answer: {}\n",
                card.question_hints.join(", ")
            ));
        }

        if !card.topic_tags.is_empty() {
            output.push_str(&format!("    Topics: {}\n", card.topic_tags.join(", ")));
        }

        output.push('\n');
    }

    // Also mention docs without cards
    let with_cards: Vec<usize> = cards.iter().map(|(idx, _)| *idx).collect();
    let without_cards: Vec<usize> = (0..ctx.doc_count())
        .filter(|i| !with_cards.contains(i))
        .collect();

    if !without_cards.is_empty() {
        output.push_str(&format!(
            "Documents without DocCards: {:?}\n",
            without_cards
                .iter()
                .map(|i| format!("doc_{}", i))
                .collect::<Vec<_>>()
        ));
    }

    ToolResult::ok(output)
}

/// Execute `find_cross` — search keywords across all documents.
///
/// Returns formatted results showing which documents matched.
pub fn find_cross(keywords: &[String], ctx: &WorkspaceContext) -> ToolResult {
    let results = ctx.find_cross_all(keywords);

    if results.is_empty() {
        return ToolResult::ok(format!(
            "No matches found for keywords: {}",
            keywords.join(", ")
        ));
    }

    let mut output = String::new();
    for (doc_idx, hits) in &results {
        let doc = ctx.doc(*doc_idx);
        let doc_name = doc.map(|d| d.doc_name).unwrap_or("unknown");
        output.push_str(&format!("Document [{}] {}:\n", doc_idx + 1, doc_name));

        for hit in hits {
            for entry in &hit.entries {
                let title = doc
                    .and_then(|d| d.node_title(entry.node_id))
                    .unwrap_or("unknown");
                let summary = doc
                    .and_then(|d| d.nav_entry(entry.node_id))
                    .map(|e| e.overview.as_str())
                    .unwrap_or("");
                output.push_str(&format!(
                    "  keyword '{}' → {} (depth {}, weight {:.2})",
                    hit.keyword, title, entry.depth, entry.weight
                ));
                if !summary.is_empty() {
                    output.push_str(&format!(" — {}", summary));
                }
                output.push('\n');
            }
        }
        output.push('\n');
    }

    ToolResult::ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{DocCard, NavigationIndex, ReasoningIndex, SectionCard};

    fn build_workspace() -> (
        Vec<crate::document::DocumentTree>,
        Vec<NavigationIndex>,
        Vec<ReasoningIndex>,
    ) {
        let tree1 = crate::document::DocumentTree::new("2024 Report", "content");
        let mut nav1 = NavigationIndex::new();
        nav1.set_doc_card(DocCard {
            title: "2024 Financial Report".to_string(),
            overview: "Annual financial statements".to_string(),
            question_hints: vec!["Revenue?".to_string()],
            topic_tags: vec!["finance".to_string(), "2024".to_string()],
            sections: vec![SectionCard {
                title: "Revenue".to_string(),
                description: "Revenue breakdown".to_string(),
                leaf_count: 5,
            }],
            total_leaves: 10,
        });

        let tree2 = crate::document::DocumentTree::new("2023 Report", "content");
        let mut nav2 = NavigationIndex::new();
        nav2.set_doc_card(DocCard {
            title: "2023 Financial Report".to_string(),
            overview: "Previous year financial statements".to_string(),
            question_hints: vec!["Sales?".to_string()],
            topic_tags: vec!["finance".to_string(), "2023".to_string()],
            sections: vec![SectionCard {
                title: "Net Sales".to_string(),
                description: "Net sales figures".to_string(),
                leaf_count: 4,
            }],
            total_leaves: 8,
        });

        (
            vec![tree1, tree2],
            vec![nav1, nav2],
            vec![ReasoningIndex::default(), ReasoningIndex::default()],
        )
    }

    #[test]
    fn test_ls_docs_shows_cards() {
        let (trees, navs, ridxs) = build_workspace();
        let docs = vec![
            crate::retrieval::agent::config::DocContext {
                tree: &trees[0],
                nav_index: &navs[0],
                reasoning_index: &ridxs[0],
                doc_name: "2024",
            },
            crate::retrieval::agent::config::DocContext {
                tree: &trees[1],
                nav_index: &navs[1],
                reasoning_index: &ridxs[1],
                doc_name: "2023",
            },
        ];
        let ctx = WorkspaceContext::new(docs);

        let result = ls_docs(&ctx);
        assert!(result.success);
        assert!(result.feedback.contains("2024 Financial Report"));
        assert!(result.feedback.contains("2023 Financial Report"));
        assert!(result.feedback.contains("Revenue"));
        assert!(result.feedback.contains("finance"));
    }

    #[test]
    fn test_ls_docs_empty() {
        let tree = crate::document::DocumentTree::new("Empty", "");
        let nav = NavigationIndex::new();
        let ridx = ReasoningIndex::default();
        let docs = vec![crate::retrieval::agent::config::DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &ridx,
            doc_name: "empty",
        }];
        let ctx = WorkspaceContext::new(docs);

        let result = ls_docs(&ctx);
        assert!(result.success);
        assert!(result.feedback.contains("No documents with DocCards"));
    }
}
