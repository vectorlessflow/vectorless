// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Incremental indexing resolver — decides what action to take for a source.
//!
//! Three-layer change detection:
//! 1. **File-level**: content fingerprint → skip if unchanged
//! 2. **Logic-level**: pipeline config fingerprint → full reprocess if changed
//! 3. **Node-level**: Merkle subtree diff → incremental update

use tracing::info;

use crate::config::PipelineOptions;
use vectorless_document::{Document, DocumentFormat, DocumentTree};
use vectorless_utils::fingerprint::Fingerprint;

/// Action to take for a source during indexing.
pub enum IndexAction {
    /// Skip entirely — content unchanged.
    Skip(SkipInfo),
    /// Full index from scratch — new file, logic changed, or force mode.
    /// If replacing an existing document, `existing_id` contains the old doc ID
    /// to clean up after the new document is successfully saved.
    FullIndex {
        /// Old document ID to remove after successful re-index (if replacing).
        existing_id: Option<String>,
    },
    /// Incremental update — content changed, pipeline unchanged.
    IncrementalUpdate {
        /// The old tree to reuse data from.
        old_tree: DocumentTree,
        /// The existing document ID (preserved across updates).
        existing_id: String,
    },
}

/// Info returned when a source is skipped.
pub struct SkipInfo {
    /// Existing document ID.
    pub doc_id: String,
    /// Document name.
    pub name: String,
    /// Document format.
    pub format: DocumentFormat,
    /// Document description.
    pub description: Option<String>,
    /// Page count.
    pub page_count: Option<usize>,
}

/// Resolve what action to take for a source file.
///
/// This is the core three-layer incremental decision:
///
/// 1. **File fingerprint**: Compare file bytes hash with stored `content_fingerprint`.
///    If equal → `Skip` (nothing changed).
///
/// 2. **Logic fingerprint**: Compare pipeline config hash with stored `logic_fingerprint`.
///    If different → `FullIndex` (processing logic changed, must reprocess everything).
///
/// 3. **Incremental**: Content changed but pipeline unchanged → `IncrementalUpdate`
///    with the old tree for partial reprocessing.
pub fn resolve_action(
    file_bytes: &[u8],
    stored_doc: &Document,
    pipeline_options: &PipelineOptions,
    format: DocumentFormat,
) -> IndexAction {
    let current_fp = Fingerprint::from_bytes(file_bytes);
    let current_fp_hex = current_fp.to_string();

    // Get the stored DocumentMeta (if present)
    let stored_meta = match stored_doc.meta.as_ref() {
        Some(m) => m,
        None => {
            // No meta → must be a very old format, full reprocess
            return IndexAction::FullIndex {
                existing_id: Some(stored_doc.doc_id.clone()),
            };
        }
    };

    // Layer 1: File-level content fingerprint
    if !stored_meta.needs_reprocessing(&current_fp_hex, pipeline_options.processing_version) {
        info!("File fingerprint unchanged, skipping");
        return IndexAction::Skip(SkipInfo {
            doc_id: stored_doc.doc_id.clone(),
            name: stored_doc.name.clone(),
            format,
            description: if stored_doc.summary.is_empty() {
                None
            } else {
                Some(stored_doc.summary.clone())
            },
            page_count: stored_doc.page_count,
        });
    }

    // Layer 2: Logic fingerprint (pipeline config changed?)
    let current_logic_fp = pipeline_options.logic_fingerprint();
    if stored_meta.logic_fingerprint != current_logic_fp.to_string()
        && !stored_meta.logic_fingerprint.is_empty()
    {
        info!("Logic fingerprint changed, full reprocess required");
        return IndexAction::FullIndex {
            existing_id: Some(stored_doc.doc_id.clone()),
        };
    }

    // Layer 3: Content changed, pipeline unchanged → incremental update
    info!("Content changed, pipeline unchanged → incremental update");
    IndexAction::IncrementalUpdate {
        old_tree: stored_doc.tree.clone(),
        existing_id: stored_doc.doc_id.clone(),
    }
}
