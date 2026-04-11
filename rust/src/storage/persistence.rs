// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Persistence utilities for saving and loading document indices.
//!
//! # Features
//!
//! - **Atomic writes**: Write to temp file, then rename for crash safety
//! - **Checksum verification**: SHA-256 checksums for data integrity
//! - **Version header**: Format version for future migrations

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::Error;
use crate::document::{DocumentTree, ReasoningIndex};
use crate::error::Result;

/// Current format version for persisted documents.
const FORMAT_VERSION: u32 = 1;

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
    #[serde(default, skip_serializing_if = "crate::utils::fingerprint::Fingerprint::is_zero")]
    pub content_fingerprint: crate::utils::fingerprint::Fingerprint,

    /// Logic fingerprint (hash of pipeline configuration used to produce this document).
    /// If the pipeline config changes, a full reprocess is needed even if content didn't change.
    #[serde(default, skip_serializing_if = "crate::utils::fingerprint::Fingerprint::is_zero")]
    pub logic_fingerprint: crate::utils::fingerprint::Fingerprint,

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
            content_fingerprint: crate::utils::fingerprint::Fingerprint::zero(),
            logic_fingerprint: crate::utils::fingerprint::Fingerprint::zero(),
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
    pub fn with_fingerprint(mut self, fp: crate::utils::fingerprint::Fingerprint) -> Self {
        self.content_fingerprint = fp;
        self
    }

    /// Set the logic fingerprint.
    pub fn with_logic_fingerprint(mut self, fp: crate::utils::fingerprint::Fingerprint) -> Self {
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
        fp: crate::utils::fingerprint::Fingerprint,
        version: u32,
        model: Option<&str>,
    ) {
        self.content_fingerprint = fp;
        self.processing_version = version;
        self.processing_model = model.map(|s| s.to_string());
        self.modified_at = chrono::Utc::now();
    }

    /// Check if the document needs reprocessing.
    pub fn needs_reprocessing(&self, current_fp: &crate::utils::fingerprint::Fingerprint, current_version: u32) -> bool {
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

/// A persisted document index containing tree and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedDocument {
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
}

impl PersistedDocument {
    /// Create a new persisted document.
    pub fn new(meta: DocumentMeta, tree: DocumentTree) -> Self {
        Self {
            meta,
            tree,
            pages: Vec::new(),
            reasoning_index: None,
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
struct PersistedWrapper<T> {
    /// Format version.
    version: u32,
    /// SHA-256 checksum of the payload.
    checksum: String,
    /// The actual data.
    payload: T,
}

/// Options for save/load operations.
#[derive(Debug, Clone)]
pub struct PersistenceOptions {
    /// Use atomic writes (temp file + rename).
    pub atomic_writes: bool,
    /// Verify checksums on load.
    pub verify_checksum: bool,
}

impl Default for PersistenceOptions {
    fn default() -> Self {
        Self {
            atomic_writes: true,
            verify_checksum: true,
        }
    }
}

impl PersistenceOptions {
    /// Create new options with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set atomic writes option.
    pub fn with_atomic_writes(mut self, enabled: bool) -> Self {
        self.atomic_writes = enabled;
        self
    }

    /// Set checksum verification option.
    pub fn with_verify_checksum(mut self, enabled: bool) -> Self {
        self.verify_checksum = enabled;
        self
    }
}

/// Calculate SHA-256 checksum of data.
fn calculate_checksum(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Save a document to a JSON file with atomic write and checksum.
///
/// # Atomic Write
///
/// When `atomic_writes` is enabled (default), this function:
/// 1. Writes to a temporary file (`.tmp` suffix)
/// 2. Renames temp file to target (atomic on most filesystems)
///
/// This prevents data corruption if the process crashes during write.
///
/// # Errors
///
/// Returns an error if:
/// - Serialization fails
/// - Cannot create temp file
/// - Write fails
/// - Rename fails
pub fn save_document(path: &Path, doc: &PersistedDocument) -> Result<()> {
    save_document_with_options(path, doc, &PersistenceOptions::default())
}

/// Save a document with custom options.
pub fn save_document_with_options(
    path: &Path,
    doc: &PersistedDocument,
    options: &PersistenceOptions,
) -> Result<()> {
    // Serialize the payload first
    let payload_bytes = serde_json::to_vec(doc).map_err(|e| Error::Serialization(e.to_string()))?;

    // Calculate checksum
    let checksum = calculate_checksum(&payload_bytes);

    // Create wrapper
    let wrapper = PersistedWrapper {
        version: FORMAT_VERSION,
        checksum,
        payload: doc.clone(),
    };

    // Serialize wrapper
    let json =
        serde_json::to_string_pretty(&wrapper).map_err(|e| Error::Serialization(e.to_string()))?;

    if options.atomic_writes {
        // Atomic write: write to temp file, then rename
        let temp_path = path.with_extension("tmp");

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }

        // Write to temp file
        {
            let file = File::create(&temp_path).map_err(Error::Io)?;
            let mut writer = BufWriter::new(file);
            writer.write_all(json.as_bytes()).map_err(Error::Io)?;
            writer.flush().map_err(Error::Io)?;
        }

        // Atomic rename
        std::fs::rename(&temp_path, path).map_err(Error::Io)?;
    } else {
        // Direct write (not atomic)
        std::fs::write(path, json).map_err(Error::Io)?;
    }

    Ok(())
}

/// Load a document from a JSON file with checksum verification.
///
/// # Checksum Verification
///
/// When `verify_checksum` is enabled (default), this function:
/// 1. Reads the file
/// 2. Parses the wrapper
/// 3. Re-serializes the payload
/// 4. Verifies the checksum matches
///
/// # Errors
///
/// Returns an error if:
/// - File doesn't exist
/// - Parse fails
/// - Checksum mismatch
/// - Version mismatch (future: migration)
pub fn load_document(path: &Path) -> Result<PersistedDocument> {
    load_document_with_options(path, &PersistenceOptions::default())
}

/// Load a document with custom options.
pub fn load_document_with_options(
    path: &Path,
    options: &PersistenceOptions,
) -> Result<PersistedDocument> {
    if !path.exists() {
        return Err(Error::DocumentNotFound(path.display().to_string()));
    }

    let file = File::open(path).map_err(Error::Io)?;
    let reader = BufReader::new(file);

    // Parse wrapper
    let wrapper: PersistedWrapper<PersistedDocument> = serde_json::from_reader(reader)
        .map_err(|e| Error::Parse(format!("Failed to parse document: {}", e)))?;

    // Check version
    if wrapper.version != FORMAT_VERSION {
        return Err(Error::Parse(format!(
            "Unsupported format version: {} (expected {})",
            wrapper.version, FORMAT_VERSION
        )));
    }

    // Verify checksum if enabled
    if options.verify_checksum {
        let payload_bytes = serde_json::to_vec(&wrapper.payload)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        let expected_checksum = calculate_checksum(&payload_bytes);

        if wrapper.checksum != expected_checksum {
            return Err(Error::Parse(format!(
                "Checksum mismatch: expected {}, got {}",
                expected_checksum, wrapper.checksum
            )));
        }
    }

    Ok(wrapper.payload)
}

/// Save the workspace index (metadata for all documents).
pub fn save_index(path: &Path, entries: &[DocumentMeta]) -> Result<()> {
    save_index_with_options(path, entries, &PersistenceOptions::default())
}

/// Save the workspace index with custom options.
pub fn save_index_with_options(
    path: &Path,
    entries: &[DocumentMeta],
    options: &PersistenceOptions,
) -> Result<()> {
    // Serialize payload
    let payload_bytes =
        serde_json::to_vec(entries).map_err(|e| Error::Serialization(e.to_string()))?;

    let checksum = calculate_checksum(&payload_bytes);

    let wrapper = PersistedWrapper {
        version: FORMAT_VERSION,
        checksum,
        payload: entries.to_vec(),
    };

    let json =
        serde_json::to_string_pretty(&wrapper).map_err(|e| Error::Serialization(e.to_string()))?;

    if options.atomic_writes {
        let temp_path = path.with_extension("tmp");

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }

        // Write to temp file
        {
            let file = File::create(&temp_path).map_err(Error::Io)?;
            let mut writer = BufWriter::new(file);
            writer.write_all(json.as_bytes()).map_err(Error::Io)?;
            writer.flush().map_err(Error::Io)?;
        }

        // Atomic rename
        std::fs::rename(&temp_path, path).map_err(Error::Io)?;
    } else {
        std::fs::write(path, json).map_err(Error::Io)?;
    }

    Ok(())
}

/// Load the workspace index.
pub fn load_index(path: &Path) -> Result<Vec<DocumentMeta>> {
    load_index_with_options(path, &PersistenceOptions::default())
}

/// Load the workspace index with custom options.
pub fn load_index_with_options(
    path: &Path,
    options: &PersistenceOptions,
) -> Result<Vec<DocumentMeta>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(path).map_err(Error::Io)?;
    let reader = BufReader::new(file);

    let wrapper: PersistedWrapper<Vec<DocumentMeta>> = serde_json::from_reader(reader)
        .map_err(|e| Error::Parse(format!("Failed to parse index: {}", e)))?;

    // Check version
    if wrapper.version != FORMAT_VERSION {
        return Err(Error::Parse(format!(
            "Unsupported format version: {} (expected {})",
            wrapper.version, FORMAT_VERSION
        )));
    }

    // Verify checksum if enabled
    if options.verify_checksum {
        let payload_bytes = serde_json::to_vec(&wrapper.payload)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        let expected_checksum = calculate_checksum(&payload_bytes);

        if wrapper.checksum != expected_checksum {
            return Err(Error::Parse(format!(
                "Checksum mismatch: expected {}, got {}",
                expected_checksum, wrapper.checksum
            )));
        }
    }

    Ok(wrapper.payload)
}

// ============================================================================
// Bytes-based serialization (for StorageBackend integration)
// ============================================================================

/// Serialize a document to bytes (JSON with checksum wrapper).
///
/// This is useful for storage backends that work with byte arrays.
pub fn save_document_to_bytes(doc: &PersistedDocument) -> Result<Vec<u8>> {
    // Serialize the payload first
    let payload_bytes = serde_json::to_vec(doc).map_err(|e| Error::Serialization(e.to_string()))?;

    // Calculate checksum
    let checksum = calculate_checksum(&payload_bytes);

    // Create wrapper
    let wrapper = PersistedWrapper {
        version: FORMAT_VERSION,
        checksum,
        payload: doc.clone(),
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
    // Parse wrapper
    let wrapper: PersistedWrapper<PersistedDocument> = serde_json::from_slice(data)
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

    Ok(wrapper.payload)
}

/// Serialize an index to bytes.
pub fn save_index_to_bytes(entries: &[DocumentMeta]) -> Result<Vec<u8>> {
    let payload_bytes =
        serde_json::to_vec(entries).map_err(|e| Error::Serialization(e.to_string()))?;

    let checksum = calculate_checksum(&payload_bytes);

    let wrapper = PersistedWrapper {
        version: FORMAT_VERSION,
        checksum,
        payload: entries.to_vec(),
    };

    serde_json::to_vec(&wrapper).map_err(|e| Error::Serialization(e.to_string()))
}

/// Deserialize an index from bytes.
pub fn load_index_from_bytes(data: &[u8]) -> Result<Vec<DocumentMeta>> {
    load_index_from_bytes_with_options(data, true)
}

/// Deserialize an index from bytes with optional checksum verification.
pub fn load_index_from_bytes_with_options(
    data: &[u8],
    verify_checksum: bool,
) -> Result<Vec<DocumentMeta>> {
    let wrapper: PersistedWrapper<Vec<DocumentMeta>> = serde_json::from_slice(data)
        .map_err(|e| Error::Parse(format!("Failed to parse index: {}", e)))?;

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

    Ok(wrapper.payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_doc(id: &str) -> PersistedDocument {
        let meta = DocumentMeta::new(id, "Test Doc", "md");
        let tree = DocumentTree::new("Root", "Content");
        PersistedDocument::new(meta, tree)
    }

    #[test]
    fn test_save_and_load_document() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.json");

        let doc = create_test_doc("doc-1");
        save_document(&path, &doc).unwrap();

        let loaded = load_document(&path).unwrap();
        assert_eq!(loaded.meta.id, "doc-1");
        assert_eq!(loaded.meta.name, "Test Doc");
    }

    #[test]
    fn test_atomic_write() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("atomic.json");

        let doc = create_test_doc("doc-atomic");
        let options = PersistenceOptions::new().with_atomic_writes(true);
        save_document_with_options(&path, &doc, &options).unwrap();

        // Temp file should not exist after save
        assert!(!path.with_extension("tmp").exists());

        let loaded = load_document(&path).unwrap();
        assert_eq!(loaded.meta.id, "doc-atomic");
    }

    #[test]
    fn test_checksum_verification() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("checksum.json");

        let doc = create_test_doc("doc-checksum");
        save_document(&path, &doc).unwrap();

        // Corrupt the file
        let content = std::fs::read_to_string(&path).unwrap();
        let corrupted = content.replace("doc-checksum", "doc-corrupted");
        std::fs::write(&path, corrupted).unwrap();

        // Load should fail with checksum error
        let result = load_document(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::Parse(_)));
    }

    #[test]
    fn test_checksum_disabled() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("no-checksum.json");

        let doc = create_test_doc("doc-no-check");
        save_document(&path, &doc).unwrap();

        // Load with checksum disabled should succeed
        let options = PersistenceOptions::new().with_verify_checksum(false);
        let result = load_document_with_options(&path, &options);
        assert!(result.is_ok());
        let loaded = result.unwrap();
        assert_eq!(loaded.meta.id, "doc-no-check");

        // Now corrupt the checksum field specifically
        let content = std::fs::read_to_string(&path).unwrap();
        // Change the checksum value but keep the payload intact
        let corrupted = content.replace(
            &calculate_checksum(&serde_json::to_vec(&doc).unwrap()),
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        std::fs::write(&path, corrupted).unwrap();

        // Load with checksum disabled should still succeed
        let result = load_document_with_options(&path, &options);
        assert!(result.is_ok());

        // Load with checksum enabled should fail
        let options_enabled = PersistenceOptions::new().with_verify_checksum(true);
        let result = load_document_with_options(&path, &options_enabled);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_nonexistent() {
        let result = load_document(Path::new("/nonexistent/path.json"));
        assert!(result.is_err());
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_save_and_load_index() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("_meta.json");

        let mut entries = Vec::new();
        entries.push(DocumentMeta::new("doc-1", "Doc 1", "md"));
        entries.push(DocumentMeta::new("doc-2", "Doc 2", "pdf"));

        save_index(&path, &entries).unwrap();

        let loaded = load_index(&path).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "doc-1");
        assert_eq!(loaded[1].format, "pdf");
    }

    #[test]
    fn test_load_empty_index() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("nonexistent.json");

        let loaded = load_index(&path).unwrap();
        assert!(loaded.is_empty());
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
        assert_eq!(checksum1.len(), 64); // SHA-256 produces 64 hex chars
    }
}
