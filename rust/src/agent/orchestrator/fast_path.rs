// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Orchestrator fast path — cross-document keyword lookup.

use tracing::info;

use crate::scoring::bm25::extract_keywords;

use super::super::config::{Config, Output, WorkspaceContext};
use super::super::context::FindHit;
use super::super::events::EventEmitter;

/// Try fast path across all documents.
pub fn fast_path(
    query: &str,
    ws: &WorkspaceContext<'_>,
    config: &Config,
    emitter: &EventEmitter,
) -> Option<Output> {
    let keywords = extract_keywords(query);
    if keywords.is_empty() {
        return None;
    }

    let cross_hits = ws.find_cross_all(&keywords);
    if cross_hits.is_empty() {
        return None;
    }

    let mut best: Option<(usize, FindHit, &crate::document::TopicEntry)> = None;
    for (doc_idx, hits) in &cross_hits {
        for hit in hits {
            for entry in &hit.entries {
                let is_better = best
                    .as_ref()
                    .map_or(true, |(_, _, best_e)| entry.weight > best_e.weight);
                if is_better && entry.weight >= config.fast_path_threshold {
                    best = Some((*doc_idx, hit.clone(), entry));
                }
            }
        }
    }

    let (doc_idx, _, best_entry) = best?;
    let doc = ws.doc(doc_idx)?;
    let content = doc.cat(best_entry.node_id).unwrap_or("").to_string();
    let title = doc
        .node_title(best_entry.node_id)
        .unwrap_or("unknown")
        .to_string();

    if content.is_empty() {
        return None;
    }

    info!(doc_idx, node = %title, weight = best_entry.weight, "Cross-doc fast path hit");
    emitter.emit_fast_path(&keywords.join(","), &title, best_entry.weight);

    Some(Output::fast_path(
        content.clone(),
        vec![super::super::config::Evidence {
            source_path: title.clone(),
            node_title: title,
            content,
            doc_name: Some(doc.doc_name.to_string()),
        }],
    ))
}
