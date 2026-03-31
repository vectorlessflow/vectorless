// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Workspace management for document collections.
//!
//! A workspace is a directory containing indexed documents and metadata.

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;

use crate::core::Result;
use super::persistence::{DocumentMeta, PersistedDocument, save_document, load_document, save_index, load_index};

const INDEX_FILE: &str = "_index.json";

/// A workspace for managing indexed documents.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Workspace {
    /// Root directory for the workspace.
    root: PathBuf,

    /// Cached document metadata (id -> meta).
    documents: HashMap<String, DocumentMeta>,

    /// Whether to lazy-load document trees.
    lazy_load: bool,
}

impl Workspace {
    /// Create a new workspace at the given path.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let root = path.into();
        fs::create_dir_all(&root)
            .map_err(|e| crate::core::Error::Io(e))?;

        let mut workspace = Self {
            root,
            documents: HashMap::new(),
            lazy_load: true,
        };

        workspace.load_index()?;
        Ok(workspace)
    }

    /// Open an existing workspace, or create if it doesn't exist.
    pub fn open(path: impl Into<PathBuf> + Clone) -> Result<Self> {
        let root = path.clone().into();
        if root.exists() {
            let mut workspace = Self {
                root,
                documents: HashMap::new(),
                lazy_load: true,
            };
            workspace.load_index()?;
            Ok(workspace)
        } else {
            Self::new(path)
        }
    }

    /// Get the workspace root path.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// List all document IDs in the workspace.
    pub fn list_documents(&self) -> Vec<&str> {
        self.documents.keys().map(|s| s.as_str()).collect()
    }

    /// Get metadata for a document.
    pub fn get_meta(&self, id: &str) -> Option<&DocumentMeta> {
        self.documents.get(id)
    }

    /// Check if a document exists.
    pub fn contains(&self, id: &str) -> bool {
        self.documents.contains_key(id)
    }

    /// Add a document to the workspace.
    pub fn add(&mut self, doc: &PersistedDocument) -> Result<()> {
        let doc_path = self.document_path(&doc.meta.id);

        // Save the document
        save_document(&doc_path, doc)?;

        // Update the index
        self.documents.insert(doc.meta.id.clone(), doc.meta.clone());
        self.save_index()?;

        Ok(())
    }

    /// Load a document from the workspace.
    pub fn load(&self, id: &str) -> Result<Option<PersistedDocument>> {
        if !self.contains(id) {
            return Ok(None);
        }

        let doc_path = self.document_path(id);
        if !doc_path.exists() {
            return Ok(None);
        }

        load_document(&doc_path).map(Some)
    }

    /// Remove a document from the workspace.
    pub fn remove(&mut self, id: &str) -> Result<bool> {
        if !self.contains(id) {
            return Ok(false);
        }

        let doc_path = self.document_path(id);
        if doc_path.exists() {
            fs::remove_file(&doc_path)
                .map_err(|e| crate::core::Error::Io(e))?;
        }

        self.documents.remove(id);
        self.save_index()?;

        Ok(true)
    }

    /// Get the number of documents in the workspace.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Check if the workspace is empty.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Get the path for a document file.
    fn document_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{}.json", id))
    }

    /// Get the path for the index file.
    fn index_path(&self) -> PathBuf {
        self.root.join(INDEX_FILE)
    }

    /// Load the workspace index from disk.
    fn load_index(&mut self) -> Result<()> {
        let index_path = self.index_path();

        if !index_path.exists() {
            // Try to rebuild from existing files
            self.rebuild_index()?;
            return Ok(());
        }

        let entries = load_index(&index_path)?;
        for entry in entries {
            self.documents.insert(entry.id.clone(), entry);
        }

        Ok(())
    }

    /// Save the workspace index to disk.
    fn save_index(&self) -> Result<()> {
        let entries: Vec<_> = self.documents.values().cloned().collect();
        save_index(&self.index_path(), &entries)
    }

    /// Rebuild the index from existing document files.
    fn rebuild_index(&mut self) -> Result<()> {
        let entries: Vec<_> = fs::read_dir(&self.root)
            .map_err(|e| crate::core::Error::Io(e))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().extension().map(|ext| ext == "json").unwrap_or(false)
            })
            .filter_map(|entry| {
                let path = entry.path();
                // Skip the index file itself
                if path.file_stem()?.to_str()? == "_index" {
                    return None;
                }
                // Try to load just the metadata
                load_document(&path).ok().map(|doc| doc.meta)
            })
            .collect();

        for entry in entries {
            self.documents.insert(entry.id.clone(), entry);
        }

        self.save_index()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_workspace_create() {
        let dir = tempdir().unwrap();
        let workspace = Workspace::new(dir.path()).unwrap();
        assert!(workspace.is_empty());
    }

    #[test]
    fn test_workspace_add_and_load() {
        use crate::core::DocumentTree;

        let dir = tempdir().unwrap();
        let mut workspace = Workspace::new(dir.path()).unwrap();

        let meta = DocumentMeta::new("test-1", "Test Doc", "md");
        let tree = DocumentTree::new("Root", "Content");
        let doc = PersistedDocument::new(meta, tree);

        workspace.add(&doc).unwrap();
        assert_eq!(workspace.len(), 1);

        let loaded = workspace.load("test-1").unwrap().unwrap();
        assert_eq!(loaded.meta.name, "Test Doc");
    }
}
