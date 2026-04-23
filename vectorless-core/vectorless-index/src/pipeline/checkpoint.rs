// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pipeline checkpoint support for resume-after-interruption.
//!
//! Saves pipeline state after each stage group completes.
//! On restart, completed stages are skipped and the pipeline resumes
//! from the first incomplete stage.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use vectorless_document::DocumentTree;
use crate::index::parse::RawNode;

use super::metrics::IndexMetrics;

/// Serializable checkpoint capturing pipeline state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineCheckpoint {
    /// Document ID being indexed.
    pub doc_id: String,

    /// SHA-256 hash of the source content.
    pub source_hash: String,

    /// Processing version at the time of checkpoint.
    pub processing_version: u32,

    /// Fingerprint of pipeline configuration.
    pub config_fingerprint: String,

    /// Names of stages that completed successfully.
    pub completed_stages: Vec<String>,

    /// Serialized context data that stages need for resume.
    pub context_data: CheckpointContextData,

    /// When this checkpoint was created.
    pub timestamp: DateTime<Utc>,
}

/// Context data that can be serialized for checkpoint persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointContextData {
    /// Raw nodes from parsing (if parse stage completed).
    pub raw_nodes: Vec<RawNode>,

    /// Built document tree (if build stage completed).
    pub tree: Option<DocumentTree>,

    /// Metrics collected so far.
    pub metrics: IndexMetrics,

    /// Page count (for PDFs).
    pub page_count: Option<usize>,

    /// Line count.
    pub line_count: Option<usize>,

    /// Document description.
    pub description: Option<String>,
}

/// Manages checkpoint persistence on disk.
pub struct CheckpointManager {
    /// Directory where checkpoints are stored.
    checkpoint_dir: PathBuf,
}

impl CheckpointManager {
    /// Create a new checkpoint manager.
    ///
    /// The directory will be created on first save if it doesn't exist.
    pub fn new(checkpoint_dir: impl Into<PathBuf>) -> Self {
        Self {
            checkpoint_dir: checkpoint_dir.into(),
        }
    }

    /// Save a checkpoint for the given document.
    pub fn save(&self, doc_id: &str, checkpoint: &PipelineCheckpoint) -> std::io::Result<()> {
        // Ensure directory exists
        std::fs::create_dir_all(&self.checkpoint_dir)?;

        let path = self.checkpoint_path(doc_id);
        let json = serde_json::to_string(checkpoint)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Write atomically: write to temp file, then rename
        let temp_path = path.with_extension("tmp");
        std::fs::write(&temp_path, json)?;
        std::fs::rename(&temp_path, &path)?;

        Ok(())
    }

    /// Load a checkpoint for the given document.
    ///
    /// Returns `None` if no checkpoint exists.
    pub fn load(&self, doc_id: &str) -> Option<PipelineCheckpoint> {
        let path = self.checkpoint_path(doc_id);
        if !path.exists() {
            return None;
        }

        let data = std::fs::read(&path).ok()?;
        match serde_json::from_slice(&data) {
            Ok(checkpoint) => Some(checkpoint),
            Err(e) => {
                warn!("Failed to deserialize checkpoint for {}: {}", doc_id, e);
                None
            }
        }
    }

    /// Remove a checkpoint after successful completion.
    pub fn clear(&self, doc_id: &str) -> std::io::Result<()> {
        let path = self.checkpoint_path(doc_id);
        if path.exists() {
            std::fs::remove_file(path)?;
            info!("Cleared checkpoint for document {}", doc_id);
        }
        Ok(())
    }

    /// Check if a checkpoint exists for the given document.
    pub fn exists(&self, doc_id: &str) -> bool {
        self.checkpoint_path(doc_id).exists()
    }

    /// Get the checkpoint file path for a document.
    fn checkpoint_path(&self, doc_id: &str) -> PathBuf {
        // Use a sanitized version of doc_id for the filename
        let safe_name = doc_id.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        self.checkpoint_dir
            .join(format!("{}.checkpoint.json", safe_name))
    }

    /// Check if a checkpoint is valid for resuming.
    ///
    /// A checkpoint is valid if:
    /// - Source hash matches (content hasn't changed)
    /// - Processing version matches (algorithm hasn't changed)
    /// - Config fingerprint matches (options haven't changed)
    pub fn is_valid_for_resume(
        checkpoint: &PipelineCheckpoint,
        source_hash: &str,
        processing_version: u32,
        config_fingerprint: &str,
    ) -> bool {
        checkpoint.source_hash == source_hash
            && checkpoint.processing_version == processing_version
            && checkpoint.config_fingerprint == config_fingerprint
    }

    /// List all checkpoint files in the directory.
    pub fn list_checkpoints(&self) -> Vec<String> {
        let mut result = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.checkpoint_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "json") {
                    if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                        // Strip .checkpoint suffix
                        if let Some(doc_id) = name.strip_suffix(".checkpoint") {
                            result.push(doc_id.to_string());
                        }
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_checkpoint() -> PipelineCheckpoint {
        PipelineCheckpoint {
            doc_id: "test-doc-123".to_string(),
            source_hash: "abc123".to_string(),
            processing_version: 1,
            config_fingerprint: "cfg-fp".to_string(),
            completed_stages: vec!["parse".to_string(), "build".to_string()],
            context_data: CheckpointContextData {
                raw_nodes: Vec::new(),
                tree: Some(DocumentTree::new("Test", "content")),
                metrics: IndexMetrics::default(),
                page_count: None,
                line_count: Some(10),
                description: None,
            },
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_save_and_load() {
        let dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(dir.path());

        let checkpoint = make_checkpoint();
        manager.save("test-doc-123", &checkpoint).unwrap();

        let loaded = manager.load("test-doc-123").unwrap();
        assert_eq!(loaded.doc_id, "test-doc-123");
        assert_eq!(loaded.completed_stages, vec!["parse", "build"]);
        assert_eq!(loaded.context_data.line_count, Some(10));
    }

    #[test]
    fn test_load_nonexistent() {
        let dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(dir.path());

        assert!(manager.load("nonexistent").is_none());
    }

    #[test]
    fn test_clear() {
        let dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(dir.path());

        let checkpoint = make_checkpoint();
        manager.save("test-doc-123", &checkpoint).unwrap();
        assert!(manager.exists("test-doc-123"));

        manager.clear("test-doc-123").unwrap();
        assert!(!manager.exists("test-doc-123"));
    }

    #[test]
    fn test_is_valid_for_resume() {
        let checkpoint = make_checkpoint();

        // Matching — valid
        assert!(CheckpointManager::is_valid_for_resume(
            &checkpoint,
            "abc123",
            1,
            "cfg-fp"
        ));

        // Different source hash — invalid
        assert!(!CheckpointManager::is_valid_for_resume(
            &checkpoint,
            "different",
            1,
            "cfg-fp"
        ));

        // Different processing version — invalid
        assert!(!CheckpointManager::is_valid_for_resume(
            &checkpoint,
            "abc123",
            2,
            "cfg-fp"
        ));

        // Different config fingerprint — invalid
        assert!(!CheckpointManager::is_valid_for_resume(
            &checkpoint,
            "abc123",
            1,
            "different"
        ));
    }

    #[test]
    fn test_list_checkpoints() {
        let dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(dir.path());

        let mut cp = make_checkpoint();
        cp.doc_id = "doc-a".to_string();
        manager.save("doc-a", &cp).unwrap();

        cp.doc_id = "doc-b".to_string();
        manager.save("doc-b", &cp).unwrap();

        let list = manager.list_checkpoints();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"doc-a".to_string()));
        assert!(list.contains(&"doc-b".to_string()));
    }

    #[test]
    fn test_roundtrip_preserves_tree() {
        let dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(dir.path());

        let mut tree = DocumentTree::new("Root", "");
        let child = tree.add_child(tree.root(), "Section 1", "Content");
        tree.set_token_count(child, 42);

        let checkpoint = PipelineCheckpoint {
            doc_id: "tree-test".to_string(),
            source_hash: "hash".to_string(),
            processing_version: 1,
            config_fingerprint: "fp".to_string(),
            completed_stages: vec!["build".to_string()],
            context_data: CheckpointContextData {
                raw_nodes: Vec::new(),
                tree: Some(tree),
                metrics: IndexMetrics::default(),
                page_count: None,
                line_count: None,
                description: None,
            },
            timestamp: Utc::now(),
        };

        manager.save("tree-test", &checkpoint).unwrap();
        let loaded = manager.load("tree-test").unwrap();

        let tree = loaded.context_data.tree.unwrap();
        assert_eq!(tree.node_count(), 2); // root + 1 child
        let child_id = tree.children(tree.root())[0];
        assert_eq!(tree.get(child_id).unwrap().title, "Section 1");
        assert_eq!(tree.get(child_id).unwrap().token_count, Some(42));
    }
}
