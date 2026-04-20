// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Fast path — keyword lookup for direct hit before full navigation.

use tracing::{debug, info};

use crate::scoring::bm25::extract_keywords;

use super::super::config::{DocContext, Evidence, Output, WorkerConfig};
use super::super::context::FindHit;
use super::super::events::EventEmitter;

/// Result of the fast-path attempt.
pub enum FastPathResult {
    /// Fast path hit — high-confidence direct answer.
    Hit(Output),
    /// Fast path miss, but ReasoningIndex returned keyword hits.
    Miss(Vec<FindHit>),
}

/// Try the fast path: extract keywords → look up in ReasoningIndex → return if confident.
pub fn fast_path(
    query: &str,
    ctx: &DocContext<'_>,
    config: &WorkerConfig,
    emitter: &EventEmitter,
) -> FastPathResult {
    let keywords = extract_keywords(query);
    if keywords.is_empty() {
        return FastPathResult::Miss(Vec::new());
    }

    let hits: Vec<FindHit> = ctx.find_all(&keywords);
    if hits.is_empty() {
        return FastPathResult::Miss(Vec::new());
    }

    let best_entry = hits
        .iter()
        .flat_map(|hit| hit.entries.iter().map(|e| (hit.keyword.clone(), e)))
        .max_by(|a, b| {
            a.1.weight
                .partial_cmp(&b.1.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

    let Some((best_kw, best)) = best_entry else {
        return FastPathResult::Miss(hits);
    };

    if best.weight < config.fast_path_threshold {
        debug!(
            keyword = %best_kw,
            weight = best.weight,
            threshold = config.fast_path_threshold,
            "Fast path: best hit below threshold"
        );
        return FastPathResult::Miss(hits);
    }

    let content = ctx.cat(best.node_id).unwrap_or("").to_string();
    let title = ctx
        .node_title(best.node_id)
        .unwrap_or("unknown")
        .to_string();

    if content.is_empty() {
        return FastPathResult::Miss(hits);
    }

    info!(keyword = %best_kw, node = %title, weight = best.weight, "Fast path hit");
    emitter.emit_worker_fast_path(ctx.doc_name, &best_kw, &title, best.weight);

    FastPathResult::Hit(Output::fast_path(
        content.clone(),
        vec![Evidence {
            source_path: title.clone(),
            node_title: title,
            content,
            doc_name: Some(ctx.doc_name.to_string()),
        }],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::DocContext;

    fn build_ctx() -> (crate::document::DocumentTree, crate::document::NavigationIndex, crate::document::ReasoningIndex) {
        let tree = crate::document::DocumentTree::new("Root", "content");
        let nav = crate::document::NavigationIndex::new();
        let ridx = crate::document::ReasoningIndex::default();
        (tree, nav, ridx)
    }

    #[test]
    fn test_fast_path_no_keywords() {
        let (tree, nav, ridx) = build_ctx();
        let ctx = DocContext { tree: &tree, nav_index: &nav, reasoning_index: &ridx, doc_name: "test" };
        let config = WorkerConfig::default();
        let emitter = EventEmitter::noop();
        let result = fast_path("the a an", &ctx, &config, &emitter);
        assert!(matches!(result, FastPathResult::Miss(ref hits) if hits.is_empty()));
    }

    #[test]
    fn test_fast_path_empty_index() {
        let (tree, nav, ridx) = build_ctx();
        let ctx = DocContext { tree: &tree, nav_index: &nav, reasoning_index: &ridx, doc_name: "test" };
        let config = WorkerConfig::default();
        let emitter = EventEmitter::noop();
        let result = fast_path("revenue finance", &ctx, &config, &emitter);
        assert!(matches!(result, FastPathResult::Miss(ref hits) if hits.is_empty()));
    }
}
