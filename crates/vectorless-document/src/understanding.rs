// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Understanding types — the core objects that define the Document Understanding Engine.
//!
//! These types form the stable public contract:
//! - [`Document`] — the unified IR artifact (the single intermediate representation)
//! - [`DocumentInfo`] — what `compile()` returns to users
//! - [`Concept`] — key concept extracted from a document
//! - [`DocumentMeta`] — storage metadata (timestamps, fingerprints, processing stats)

use serde::{Deserialize, Serialize};

use super::toc::TocNode;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Current IR schema version.
///
/// Increment when the serialized structure changes in a backward-incompatible way.
/// Old IRs will be detected via `schema_version < CURRENT_SCHEMA_VERSION`.
pub const CURRENT_SCHEMA_VERSION: u32 = 3;

// ---------------------------------------------------------------------------
// Document — unified IR artifact
// ---------------------------------------------------------------------------

/// A compiled document — the single Intermediate Representation (IR) artifact.
///
/// Produced by the compile pipeline, persisted to disk, and consumed by the agent
/// during retrieval. This is the authoritative data structure: no other intermediate
/// types exist between the pipeline output and the navigator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    // ── Identity ──
    /// Schema version for IR format compatibility.
    #[serde(default)]
    pub schema_version: u32,

    /// Unique document identifier.
    pub doc_id: String,

    /// Document name/title.
    pub name: String,

    /// Document format ("pdf", "markdown", "docx").
    pub format: String,

    /// Source file path (if compiled from a file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,

    // ── Indexes ──
    /// Hierarchical semantic tree.
    pub tree: super::tree::DocumentTree,

    /// Pre-computed navigation structure.
    pub nav_index: super::navigation::NavigationIndex,

    /// Keyword / topic / section summaries.
    pub reasoning_index: super::reasoning::ReasoningIndex,

    // ── Compile results ──
    /// Document-level summary.
    pub summary: String,

    /// Key concepts the engine identified.
    #[serde(default)]
    pub concepts: Vec<Concept>,

    // ── Agent acceleration data ──
    /// Pre-computed query routing table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_routes: Option<super::query_route::QueryRoutingTable>,

    /// Reasoning chain index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_index: Option<super::chain::ChainIndex>,

    /// Content overlap map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_overlap: Option<super::overlap::ContentOverlapMap>,

    /// Per-node evidence quality scores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_scores: Option<super::evidence::EvidenceScoreMap>,

    // ── Metadata ──
    /// Page count (for PDFs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<usize>,

    /// Storage metadata (timestamps, fingerprints, processing stats).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<DocumentMeta>,
}

// ---------------------------------------------------------------------------
// DocumentMeta — storage metadata embedded in IR
// ---------------------------------------------------------------------------

/// Metadata for a compiled document IR file.
///
/// Holds timestamps, content fingerprints, and processing statistics.
/// Used for incremental recompilation and diagnostic purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    /// Creation timestamp.
    #[serde(default = "default_now")]
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Last modified timestamp.
    #[serde(default = "default_now")]
    pub modified_at: chrono::DateTime<chrono::Utc>,

    /// Content fingerprint for change detection (hex-encoded BLAKE2b hash).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub content_fingerprint: String,

    /// Logic fingerprint (hash of pipeline configuration).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub logic_fingerprint: String,

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

    /// Line count (for text files).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_count: Option<usize>,
}

fn default_now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

impl DocumentMeta {
    /// Create new metadata with current timestamps.
    pub fn new() -> Self {
        let now = chrono::Utc::now();
        Self {
            created_at: now,
            modified_at: now,
            content_fingerprint: String::new(),
            logic_fingerprint: String::new(),
            processing_version: 0,
            node_count: 0,
            total_summary_tokens: 0,
            processing_model: None,
            processing_duration_ms: 0,
            line_count: None,
        }
    }

    /// Set the content fingerprint.
    pub fn with_content_fingerprint(mut self, fp: impl Into<String>) -> Self {
        self.content_fingerprint = fp.into();
        self
    }

    /// Set the logic fingerprint.
    pub fn with_logic_fingerprint(mut self, fp: impl Into<String>) -> Self {
        self.logic_fingerprint = fp.into();
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

    /// Check if the document needs reprocessing.
    pub fn needs_reprocessing(&self, current_fp: &str, current_version: u32) -> bool {
        if self.processing_version == 0 {
            return true;
        }
        if self.processing_version < current_version {
            return true;
        }
        if self.content_fingerprint != current_fp {
            return true;
        }
        false
    }
}

impl Default for DocumentMeta {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// DocumentInfo — what compile() returns to users
// ---------------------------------------------------------------------------

/// The engine's understanding of a document — returned by `compile()`.
///
/// Rich enough for users to confirm the engine "got it right":
/// summary, structure (TOC), and key concepts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentInfo {
    /// Unique document identifier.
    pub doc_id: String,
    /// Document name.
    pub name: String,
    /// Document format ("pdf", "markdown", "docx").
    pub format: String,
    /// Document-level summary — what this document is about.
    pub summary: String,
    /// Table of contents — the document's structure as the engine sees it.
    pub structure: TocNode,
    /// Key concepts the engine identified.
    pub concepts: Vec<Concept>,
    /// Number of sections in the document.
    pub section_count: usize,
    /// Page count (for PDFs).
    pub page_count: Option<usize>,
}

// ---------------------------------------------------------------------------
// Document impl
// ---------------------------------------------------------------------------

impl Document {
    /// Get node content by ID (Agent `cat` command).
    pub fn cat(&self, node_id: super::node::NodeId) -> Option<&str> {
        self.tree.get(node_id).map(|n| n.content.as_str())
    }

    /// Find nodes containing a keyword in title or content.
    pub fn find(&self, keyword: &str) -> Vec<(super::node::NodeId, &str)> {
        let kw = keyword.to_lowercase();
        self.tree
            .traverse()
            .iter()
            .filter_map(|&id| {
                let node = self.tree.get(id)?;
                if node.title.to_lowercase().contains(&kw)
                    || node.content.to_lowercase().contains(&kw)
                {
                    Some((id, node.title.as_str()))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get node title by ID.
    pub fn node_title(&self, node_id: super::node::NodeId) -> Option<&str> {
        self.tree.get(node_id).map(|n| n.title.as_str())
    }

    /// Number of sections in the tree.
    pub fn section_count(&self) -> usize {
        self.tree.node_count()
    }

    /// Produce the public DocumentInfo view of this document.
    pub fn info(&self) -> DocumentInfo {
        let toc = super::toc::TocView::new().generate(&self.tree);
        DocumentInfo {
            doc_id: self.doc_id.clone(),
            name: self.name.clone(),
            format: self.format.clone(),
            summary: self.summary.clone(),
            structure: toc,
            concepts: self.concepts.clone(),
            section_count: self.section_count(),
            page_count: self.page_count,
        }
    }
}

// ---------------------------------------------------------------------------
// Concept
// ---------------------------------------------------------------------------

/// A key concept extracted from a document.
///
/// Produced during the compile pipeline's concept extraction step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    /// Concept name (e.g., "capacitor derating").
    pub name: String,
    /// One-sentence explanation.
    pub summary: String,
    /// Which sections this concept appears in.
    pub sections: Vec<String>,
}

// ---------------------------------------------------------------------------
// IngestInput — what compile() takes
// ---------------------------------------------------------------------------

/// Input to `compile()` — the document to be understood.
#[derive(Debug, Clone)]
pub enum IngestInput {
    /// Compile from a file path.
    Path(std::path::PathBuf),
    /// Compile from raw bytes.
    Bytes {
        /// Document name.
        name: String,
        /// Raw document bytes.
        data: Vec<u8>,
        /// Document format.
        format: super::format::DocumentFormat,
    },
    /// Compile from a text string.
    Text {
        /// Document name.
        name: String,
        /// Document content.
        content: String,
    },
    /// Compile from pre-parsed raw nodes.
    ///
    /// Skips the parse stage — the pipeline starts from tree building.
    /// Use this when the caller has already structured the document.
    PreParsed {
        /// Document name.
        name: String,
        /// Pre-parsed raw nodes.
        nodes: Vec<RawNodeInput>,
    },
}

/// A raw node for [`IngestInput::PreParsed`].
///
/// Simplified version of `RawNode` for external API — callers construct
/// these from Python or other languages.
#[derive(Debug, Clone)]
pub struct RawNodeInput {
    /// Node title (e.g., section heading or file path).
    pub title: String,
    /// Node content.
    pub content: String,
    /// Hierarchy level (0 = root, 1 = top-level, etc.).
    pub level: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concept_serialization() {
        let concept = Concept {
            name: "capacitor derating".into(),
            summary: "Reducing capacitor specs for reliability".into(),
            sections: vec!["Section 3.2".into()],
        };
        let json = serde_json::to_string(&concept).unwrap();
        assert!(json.contains("capacitor derating"));
    }

    #[test]
    fn test_document_meta_default() {
        let meta = DocumentMeta::new();
        assert_eq!(meta.processing_version, 0);
        assert!(meta.content_fingerprint.is_empty());
    }

    #[test]
    fn test_document_meta_needs_reprocessing() {
        let meta = DocumentMeta::new();
        assert!(meta.needs_reprocessing("abc", 1));

        let meta = DocumentMeta::new()
            .with_content_fingerprint("abc")
            .with_processing_version(1);
        assert!(!meta.needs_reprocessing("abc", 1));
        assert!(meta.needs_reprocessing("def", 1));
        assert!(meta.needs_reprocessing("abc", 2));
    }
}
