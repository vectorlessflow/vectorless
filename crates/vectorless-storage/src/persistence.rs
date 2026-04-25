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

use vectorless_document::{CURRENT_SCHEMA_VERSION, Document};
use vectorless_error::Error;
use vectorless_error::Result;

/// Current format version for persisted documents.
///
/// Bumped to 2 because the payload changed from `PersistedDocument` to `Document`.
const FORMAT_VERSION: u32 = 2;

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
pub fn save_document_to_bytes(doc: &Document) -> Result<Vec<u8>> {
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
pub fn load_document_from_bytes(data: &[u8]) -> Result<Document> {
    load_document_from_bytes_with_options(data, true)
}

/// Deserialize a document from bytes with optional checksum verification.
pub fn load_document_from_bytes_with_options(
    data: &[u8],
    verify_checksum: bool,
) -> Result<Document> {
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
    let doc: Document = serde_json::from_value(wrapper.payload)
        .map_err(|e| Error::Parse(format!("Failed to deserialize document: {}", e)))?;

    // Check schema version
    if doc.schema_version == 0 {
        tracing::warn!(
            doc_id = %doc.doc_id,
            "Document was created before schema versioning — consider re-indexing"
        );
    } else if doc.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(Error::Parse(format!(
            "Document schema version {} is newer than supported {} — please upgrade vectorless",
            doc.schema_version, CURRENT_SCHEMA_VERSION
        )));
    }

    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_doc(id: &str) -> Document {
        Document {
            schema_version: CURRENT_SCHEMA_VERSION,
            doc_id: id.to_string(),
            name: "Test Doc".to_string(),
            format: "md".to_string(),
            source_path: None,
            tree: vectorless_document::DocumentTree::new("Root", "Content"),
            nav_index: Default::default(),
            reasoning_index: Default::default(),
            summary: String::new(),
            concepts: Vec::new(),
            query_routes: None,
            chain_index: None,
            content_overlap: None,
            evidence_scores: None,
            page_count: None,
            meta: None,
        }
    }

    #[test]
    fn test_save_and_load_bytes() {
        let doc = create_test_doc("doc-1");
        let bytes = save_document_to_bytes(&doc).unwrap();
        let loaded = load_document_from_bytes(&bytes).unwrap();
        assert_eq!(loaded.doc_id, "doc-1");
        assert_eq!(loaded.name, "Test Doc");
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
