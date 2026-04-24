// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Understanding types — the core objects that define the Document Understanding Engine.
//!
//! These types form the stable public contract:
//! - [`Document`] — the unified post-compile artifact (internal first-class citizen)
//! - [`DocumentInfo`] — what `compile()` returns to users
//! - [`Concept`] — key concept extracted from a document

use serde::{Deserialize, Serialize};

use super::toc::TocNode;

// ---------------------------------------------------------------------------
// Document — unified post-compile artifact
// ---------------------------------------------------------------------------

/// A compiled document — the core artifact of the compile pipeline.
///
/// This is what `compile()` produces internally.
/// It unifies tree + navigation index + reasoning index + summary + concepts
/// + agent acceleration data into a single first-class type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
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
    /// Number of sections in the tree.
    #[serde(default)]
    pub section_count: usize,
}

// ---------------------------------------------------------------------------
// DocumentInfo — what ingest() returns to users
// ---------------------------------------------------------------------------

/// The engine's understanding of a document — returned by `ingest()`.
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
        self.section_count
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
            section_count: self.section_count,
            page_count: self.page_count,
        }
    }
}

// ---------------------------------------------------------------------------
// Concept
// ---------------------------------------------------------------------------

/// A key concept extracted from a document.
///
/// Produced during the ingest pipeline's final concept extraction step.
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
// IngestInput — what ingest() takes
// ---------------------------------------------------------------------------

/// Input to `ingest()` — the document to be understood.
#[derive(Debug, Clone)]
pub enum IngestInput {
    /// Index from a file path.
    Path(std::path::PathBuf),
    /// Index from raw bytes.
    Bytes {
        /// Document name.
        name: String,
        /// Raw document bytes.
        data: Vec<u8>,
        /// Document format.
        format: super::format::DocumentFormat,
    },
    /// Index from a text string.
    Text {
        /// Document name.
        name: String,
        /// Document content.
        content: String,
    },
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
}
