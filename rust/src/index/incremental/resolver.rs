// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Incremental indexing resolver — decides what action to take for a source.
//!
//! Three-layer change detection:
//! 1. **File-level**: content fingerprint → skip if unchanged
//! 2. **Logic-level**: pipeline config fingerprint → full reprocess if changed
//! 3. **Node-level**: Merkle subtree diff → incremental update

use tracing::info;

use crate::document::DocumentTree;
use crate::storage::PersistedDocument;
use crate::utils::fingerprint::Fingerprint;
use crate::index::config::PipelineOptions;
use crate::parser::DocumentFormat;

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
    stored_doc: &PersistedDocument,
    pipeline_options: &PipelineOptions,
    format: DocumentFormat,
) -> IndexAction {
    let current_fp = Fingerprint::from_bytes(file_bytes);

    // Layer 1: File-level content fingerprint
    if !stored_doc.meta.needs_reprocessing(&current_fp, pipeline_options.processing_version) {
        info!("File fingerprint unchanged, skipping");
        return IndexAction::Skip(SkipInfo {
            doc_id: stored_doc.meta.id.clone(),
            name: stored_doc.meta.name.clone(),
            format,
            description: stored_doc.meta.description.clone(),
            page_count: stored_doc.meta.page_count,
        });
    }

    // Layer 2: Logic fingerprint (pipeline config changed?)
    let current_logic_fp = pipeline_options.logic_fingerprint();
    if stored_doc.meta.logic_fingerprint != current_logic_fp
        && !stored_doc.meta.logic_fingerprint.is_zero()
    {
        info!("Logic fingerprint changed, full reprocess required");
        return IndexAction::FullIndex {
            existing_id: Some(stored_doc.meta.id.clone()),
        };
    }

    // Layer 3: Content changed, pipeline unchanged → incremental update
    info!("Content changed, pipeline unchanged → incremental update");
    IndexAction::IncrementalUpdate {
        old_tree: stored_doc.tree.clone(),
        existing_id: stored_doc.meta.id.clone(),
    }
}
