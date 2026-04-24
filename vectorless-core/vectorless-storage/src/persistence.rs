// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Persistence utilities for saving and loading compiled documents.
//!
//! # Features
//!
//! - **Atomic writes**: Write to temp file, then rename for crash safety
//! - **Checksum verification**: SHA-256 checksums for data integrity
//! - **Version header**: Format version for future migrations

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

use vectorless_document::{
    ChainIndex, ContentOverlapMap, DocumentTree, EvidenceScoreMap, NavigationIndex,
    QueryRoutingTable, ReasoningIndex,
};
use vectorless_error::Error;
use vectorless_error::Result;

/// Current format version for persisted documents.
const FORMAT_VERSION: u32 = 1;

/// Current schema version for `PersistedDocument`.
///
/// Increment this when the document structure changes in a
/// backward-incompatible way (e.g. field renames, new required fields).
/// Old documents will be detected and logged as stale on load.
const SCHEMA_VERSION: u32 = 2;

/// Metadata for a persisted document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    /// Unique document identifier.
    pub id: String,

    /// Document name/title.
    pub name: String,

    /// Document format (md, pdf, etc.).
    pub format: String,

    /// Source file path.
    pub source_path: Option<PathBuf>,

    /// Document description.
    pub description: Option<String>,

    /// Page count (for PDFs).
    pub page_count: Option<usize>,

    /// Line count (for text files).
    pub line_count: Option<usize>,

    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Last modified timestamp.
    pub modified_at: chrono::DateTime<chrono::Utc>,

    // === Processing State (for incremental updates) ===
    /// Content fingerprint for change detection.
    #[serde(
        default,
        skip_serializing_if = "vectorless_utils::fingerprint::Fingerprint::is_zero"
    )]
    pub content_fingerprint: vectorless_utils::fingerprint::Fingerprint,

    /// Logic fingerprint (hash of pipeline configuration used to produce this document).
    /// If the pipeline config changes, a full reprocess is needed even if content didn't change.
    #[serde(
        default,
        skip_serializing_if = "vectorless_utils::fingerprint::Fingerprint::is_zero"
    )]
    pub logic_fingerprint: vectorless_utils::fingerprint::Fingerprint,

    /// Processing version (incremented when algorithm changes).
    #[serde(default)]
    pub processing_version: u32,

    /// Node count in the tree.
    #[serde(default)]
    pub node_count: usize,

    /// Total tokens in summaries.
    #[serde(default)]
    pub total_summary_tokens: usize,

    /// LLM model used for processing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processing_model: Option<String>,

    /// Last processing duration in milliseconds.
    #[serde(default)]
    pub processing_duration_ms: u64,
}

impl DocumentMeta {
    /// Create new document metadata.
    pub fn new(id: impl Into<String>, name: impl Into<String>, format: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            format: format.into(),
            source_path: None,
            description: None,
            page_count: None,
            line_count: None,
            created_at: now,
            modified_at: now,
            content_fingerprint: vectorless_utils::fingerprint::Fingerprint::zero(),
            logic_fingerprint: vectorless_utils::fingerprint::Fingerprint::zero(),
            processing_version: 0,
            node_count: 0,
            total_summary_tokens: 0,
            processing_model: None,
            processing_duration_ms: 0,
        }
    }

    /// Set the source path.
    pub fn with_source_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    /// Set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the content fingerprint.
    pub fn with_fingerprint(mut self, fp: vectorless_utils::fingerprint::Fingerprint) -> Self {
        self.content_fingerprint = fp;
        self
    }

    /// Set the logic fingerprint.
    pub fn with_logic_fingerprint(
        mut self,
        fp: vectorless_utils::fingerprint::Fingerprint,
    ) -> Self {
        self.logic_fingerprint = fp;
        self
    }

    /// Set the processing version.
    pub fn with_processing_version(mut self, version: u32) -> Self {
        self.processing_version = version;
        self
    }

    /// Set the processing model.
    pub fn with_processing_model(mut self, model: impl Into<String>) -> Self {
        self.processing_model = Some(model.into());
        self
    }

    /// Update processing statistics.
    pub fn update_processing_stats(
        &mut self,
        node_count: usize,
        summary_tokens: usize,
        duration_ms: u64,
    ) {
        self.node_count = node_count;
        self.total_summary_tokens = summary_tokens;
        self.processing_duration_ms = duration_ms;
        self.modified_at = chrono::Utc::now();
    }

    /// Mark as processed with given fingerprint and version.
    pub fn mark_processed(
        &mut self,
        fp: vectorless_utils::fingerprint::Fingerprint,
        version: u32,
        model: Option<&str>,
    ) {
        self.content_fingerprint = fp;
        self.processing_version = version;
        self.processing_model = model.map(|s| s.to_string());
        self.modified_at = chrono::Utc::now();
    }

    /// Check if the document needs reprocessing.
    pub fn needs_reprocessing(
        &self,
        current_fp: &vectorless_utils::fingerprint::Fingerprint,
        current_version: u32,
    ) -> bool {
        // Never processed
        if self.processing_version == 0 {
            return true;
        }

        // Algorithm version changed
        if self.processing_version < current_version {
            return true;
        }

        // Content changed
        if &self.content_fingerprint != current_fp {
            return true;
        }

        false
    }
}

/// A persisted compiled document containing tree, indexes, and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedDocument {
    /// Schema version — incremented on backward-incompatible changes.
    /// Old documents default to `0` via serde when the field is absent.
    #[serde(default)]
    pub schema_version: u32,

    /// Document metadata.
    pub meta: DocumentMeta,

    /// The document tree structure.
    pub tree: DocumentTree,

    /// Per-page content (for PDFs).
    #[serde(default)]
    pub pages: Vec<PageContent>,

    /// Pre-computed reasoning index for retrieval acceleration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_index: Option<ReasoningIndex>,

    /// Navigation index for Agent-based retrieval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation_index: Option<NavigationIndex>,

    /// Key concepts extracted from the document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concepts: Vec<vectorless_document::Concept>,

    // ── Agent acceleration data ──

    /// Pre-computed query routing table for Agent acceleration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_routes: Option<QueryRoutingTable>,

    /// Reasoning chain index for cross-section navigation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_index: Option<ChainIndex>,

    /// Content overlap map to prevent duplicate visits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_overlap: Option<ContentOverlapMap>,

    /// Per-node evidence quality scores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_scores: Option<EvidenceScoreMap>,
}

impl PersistedDocument {
    /// Create a new persisted document.
    pub fn new(meta: DocumentMeta, tree: DocumentTree) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            meta,
            tree,
            pages: Vec::new(),
            reasoning_index: None,
            navigation_index: None,
            concepts: Vec::new(),
            query_routes: None,
            chain_index: None,
            content_overlap: None,
            evidence_scores: None,
        }
    }

    /// Add page content.
    pub fn add_page(&mut self, page: usize, content: impl Into<String>) {
        self.pages.push(PageContent {
            page,
            content: content.into(),
        });
    }
}

/// Content for a single page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageContent {
    /// Page number (1-based).
    pub page: usize,

    /// Page text content.
    pub content: String,
}

/// Wrapper for persisted data with checksum.
#[derive(Debug, Serialize, Deserialize)]
struct PersistedWrapper {
    /// Format version.
    version: u32,
    /// SHA-256 checksum of the payload.
    checksum: String,
    /// The actual data as raw JSON value (avoids re-serialization drift).
    payload: serde_json::Value,
}

/// Calculate SHA-256 checksum of data.
fn calculate_checksum(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

// ============================================================================
// Bytes-based serialization (for StorageBackend integration)
// ============================================================================

/// Serialize a document to bytes (JSON with checksum wrapper).
///
/// This is useful for storage backends that work with byte arrays.
pub fn save_document_to_bytes(doc: &PersistedDocument) -> Result<Vec<u8>> {
    // Serialize to serde_json::Value first
    let payload_value =
        serde_json::to_value(doc).map_err(|e| Error::Serialization(e.to_string()))?;

    // Calculate checksum on the Value's canonical bytes
    let payload_bytes =
        serde_json::to_vec(&payload_value).map_err(|e| Error::Serialization(e.to_string()))?;
    let checksum = calculate_checksum(&payload_bytes);

    // Create wrapper
    let wrapper = PersistedWrapper {
        version: FORMAT_VERSION,
        checksum,
        payload: payload_value,
    };

    // Serialize wrapper
    serde_json::to_vec(&wrapper).map_err(|e| Error::Serialization(e.to_string()))
}

/// Deserialize a document from bytes.
///
/// Verifies checksum by default.
pub fn load_document_from_bytes(data: &[u8]) -> Result<PersistedDocument> {
    load_document_from_bytes_with_options(data, true)
}

/// Deserialize a document from bytes with optional checksum verification.
pub fn load_document_from_bytes_with_options(
    data: &[u8],
    verify_checksum: bool,
) -> Result<PersistedDocument> {
    // Parse wrapper (payload is serde_json::Value)
    let wrapper: PersistedWrapper = serde_json::from_slice(data)
        .map_err(|e| Error::Parse(format!("Failed to parse document: {}", e)))?;

    // Check version
    if wrapper.version != FORMAT_VERSION {
        return Err(Error::VersionMismatch(format!(
            "Expected version {}, got {}",
            FORMAT_VERSION, wrapper.version
        )));
    }

    // Verify checksum if enabled
    if verify_checksum {
        let payload_bytes = serde_json::to_vec(&wrapper.payload)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        let expected_checksum = calculate_checksum(&payload_bytes);

        if wrapper.checksum != expected_checksum {
            return Err(Error::ChecksumMismatch(format!(
                "Expected {}, got {}",
                expected_checksum, wrapper.checksum
            )));
        }
    }

    // Deserialize Value to target type
    let doc: PersistedDocument = serde_json::from_value(wrapper.payload)
        .map_err(|e| Error::Parse(format!("Failed to deserialize document: {}", e)))?;

    // Check schema version
    if doc.schema_version == 0 {
        tracing::warn!(
            doc_id = %doc.meta.id,
            "Document was created before schema versioning — consider re-indexing"
        );
    } else if doc.schema_version > SCHEMA_VERSION {
        return Err(Error::Parse(format!(
            "Document schema version {} is newer than supported {} — please upgrade vectorless",
            doc.schema_version, SCHEMA_VERSION
        )));
    }

    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_doc(id: &str) -> PersistedDocument {
        let meta = DocumentMeta::new(id, "Test Doc", "md");
        let tree = DocumentTree::new("Root", "Content");
        PersistedDocument::new(meta, tree)
    }

    #[test]
    fn test_save_and_load_bytes() {
        let doc = create_test_doc("doc-1");
        let bytes = save_document_to_bytes(&doc).unwrap();
        let loaded = load_document_from_bytes(&bytes).unwrap();
        assert_eq!(loaded.meta.id, "doc-1");
        assert_eq!(loaded.meta.name, "Test Doc");
    }

    #[test]
    fn test_checksum_verification_bytes() {
        let doc = create_test_doc("doc-check");
        let bytes = save_document_to_bytes(&doc).unwrap();

        // Corrupt a byte
        let mut corrupted = bytes.clone();
        corrupted[10] ^= 0xFF;

        let result = load_document_from_bytes(&corrupted);
        assert!(result.is_err());
    }

    #[test]
    fn test_checksum_disabled_bytes() {
        let doc = create_test_doc("doc-no-check");
        let bytes = save_document_to_bytes(&doc).unwrap();

        // Load with checksum disabled should succeed even for raw bytes
        let result = load_document_from_bytes_with_options(&bytes, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_checksum_calculation() {
        let data1 = b"test data";
        let data2 = b"test data";
        let data3 = b"different data";

        let checksum1 = calculate_checksum(data1);
        let checksum2 = calculate_checksum(data2);
        let checksum3 = calculate_checksum(data3);

        assert_eq!(checksum1, checksum2);
        assert_ne!(checksum1, checksum3);
        assert_eq!(checksum1.len(), 64);
    }
}
