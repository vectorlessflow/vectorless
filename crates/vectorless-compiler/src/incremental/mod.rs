// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Incremental indexing support.
//!
//! This module provides functionality to incrementally update
//! an existing document index when the source document changes.
//!
//! # Features
//!
//! - **Fine-grained change detection**: Uses subtree fingerprints to identify
//!   exactly which nodes changed
//! - **Processing version tracking**: Automatically reprocesses when algorithm
//!   versions change
//! - **Partial updates**: Only reprocess changed nodes

mod detector;
mod resolver;

pub use detector::ChangeDetector;
pub use resolver::{IndexAction, SkipInfo, resolve_action};
use std::collections::HashMap;
use vectorless_document::{DocumentTree, NodeId};
use vectorless_utils::fingerprint::{Fingerprint, Fingerprinter};

/// Reuse summaries from old tree for unchanged nodes in the new tree.
///
/// Uses `ChangeDetector` to find which nodes changed, then copies
/// summaries from old tree nodes with matching titles that are unchanged.
///
/// Returns a map of `title -> summary` for reusable summaries.
pub fn compute_reusable_summaries(
    old_tree: &DocumentTree,
    new_tree: &DocumentTree,
) -> HashMap<String, String> {
    let detector = ChangeDetector::new();
    let changes = detector.detect_changes(old_tree, new_tree);

    let changed_titles: std::collections::HashSet<String> = changes
        .modified
        .iter()
        .chain(changes.restructured.iter())
        .chain(changes.added.iter())
        .chain(changes.removed.iter())
        .map(|c| c.title.clone())
        .collect();

    let mut reusable = HashMap::new();
    for node_id in old_tree.traverse() {
        if let Some(node) = old_tree.get(node_id) {
            if !changed_titles.contains(&node.title) && !node.summary.is_empty() {
                reusable.insert(node.title.clone(), node.summary.clone());
            }
        }
    }
    reusable
}

/// Apply reusable summaries to a new tree.
///
/// For each node in `new_tree` whose title matches a key in `summaries`,
/// sets the node's summary from the map.
///
/// Returns the number of summaries applied.
pub fn apply_reusable_summaries(
    new_tree: &mut DocumentTree,
    summaries: &HashMap<String, String>,
) -> usize {
    let mut applied = 0;
    for node_id in new_tree.traverse() {
        if let Some(node) = new_tree.get(node_id) {
            if node.summary.is_empty() {
                if let Some(summary) = summaries.get(&node.title) {
                    new_tree.set_summary(node_id, summary);
                    applied += 1;
                }
            }
        }
    }
    applied
}

// ---------------------------------------------------------------------------
// Content-addressed enrichment reuse (robust for code: handles duplicate
// symbol names, and reuses keywords/question_hints — not just the summary).
// ---------------------------------------------------------------------------

/// Full per-node enrichment that can be carried over from a previous compile.
#[derive(Clone)]
pub struct NodeEnrichment {
    /// LLM-generated summary.
    pub summary: String,
    /// Routing keywords (topic tags).
    pub routing_keywords: Vec<String>,
    /// Typical questions this subtree can answer.
    pub question_hints: Vec<String>,
}

/// Content-only subtree fingerprint per node.
///
/// Unlike the change-detector fingerprint, this excludes the positional
/// `node_id`, so it is stable under insertions/removals elsewhere in the tree
/// and identical for nodes with identical content (e.g. two same-named methods
/// only match if their bodies match). This is what makes reuse correct for code.
fn content_fps(tree: &DocumentTree) -> HashMap<NodeId, Fingerprint> {
    fn rec(tree: &DocumentTree, id: NodeId, map: &mut HashMap<NodeId, Fingerprint>) -> Fingerprint {
        let (title, content) = match tree.get(id) {
            Some(n) => (n.title.clone(), n.content.clone()),
            None => (String::new(), String::new()),
        };
        let content_fp = Fingerprinter::new()
            .with_str(&title)
            .with_str(&content)
            .into_fingerprint();
        let children = tree.children(id);
        let fp = if children.is_empty() {
            content_fp
        } else {
            let mut fpr = Fingerprinter::new();
            fpr.write_fingerprint(&content_fp);
            for child in children {
                let child_fp = rec(tree, child, map);
                fpr.write_fingerprint(&child_fp);
            }
            fpr.into_fingerprint()
        };
        map.insert(id, fp);
        fp
    }

    let mut map = HashMap::new();
    rec(tree, tree.root(), &mut map);
    map
}

/// Build a content-addressed index of reusable enrichment from a previous tree.
///
/// Key = content-only subtree fingerprint (base64). Only nodes that actually
/// carry enrichment are included.
pub fn build_enrichment_index(old_tree: &DocumentTree) -> HashMap<String, NodeEnrichment> {
    let fps = content_fps(old_tree);
    let mut index: HashMap<String, NodeEnrichment> = HashMap::new();

    for node_id in old_tree.traverse() {
        let Some(node) = old_tree.get(node_id) else {
            continue;
        };
        if node.summary.is_empty()
            && node.routing_keywords.is_empty()
            && node.question_hints.is_empty()
        {
            continue;
        }
        if let Some(fp) = fps.get(&node_id) {
            index
                .entry(fp.to_base64())
                .or_insert_with(|| NodeEnrichment {
                    summary: node.summary.clone(),
                    routing_keywords: node.routing_keywords.clone(),
                    question_hints: node.question_hints.clone(),
                });
        }
    }

    index
}

/// Apply reusable enrichment to a new tree, matched by content fingerprint.
///
/// Reuses summary + routing_keywords + question_hints for unchanged nodes.
/// Returns the number of nodes that reused prior enrichment.
pub fn apply_enrichment_index(
    new_tree: &mut DocumentTree,
    index: &HashMap<String, NodeEnrichment>,
) -> usize {
    if index.is_empty() {
        return 0;
    }

    let fps = content_fps(new_tree);
    let updates: Vec<(NodeId, NodeEnrichment)> = new_tree
        .traverse()
        .into_iter()
        .filter_map(|id| {
            let node = new_tree.get(id)?;
            if !node.summary.is_empty() {
                return None; // already enriched this run
            }
            let fp = fps.get(&id)?;
            index.get(&fp.to_base64()).cloned().map(|e| (id, e))
        })
        .collect();

    let applied = updates.len();
    for (id, enrichment) in updates {
        new_tree.set_summary(id, &enrichment.summary);
        if let Some(node) = new_tree.get_mut(id) {
            if !enrichment.routing_keywords.is_empty() {
                node.routing_keywords = enrichment.routing_keywords;
            }
            if !enrichment.question_hints.is_empty() {
                node.question_hints = enrichment.question_hints;
            }
        }
    }
    applied
}
