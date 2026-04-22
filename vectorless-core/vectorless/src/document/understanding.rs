// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Understanding types — the core objects that define the Document Understanding Engine.
//!
//! These types form the stable public contract:
//! - [`Document`] — the unified post-ingest artifact (internal first-class citizen)
//! - [`DocumentInfo`] — what `ingest()` returns to users
//! - [`Concept`] — key concept extracted from a document
//! - [`Answer`] — what `ask()` returns
//! - [`Evidence`] — proof trail for an answer
//! - [`ReasoningTrace`] / [`TraceStep`] — always-mandatory reasoning trace

use serde::{Deserialize, Serialize};

use super::toc::TocNode;

// ---------------------------------------------------------------------------
// Document — unified post-ingest artifact
// ---------------------------------------------------------------------------

/// A understood document — the core artifact of the understand phase.
///
/// This is what `ingest()` produces internally and what `ask()` consumes.
/// It unifies tree + navigation index + reasoning index + summary + concepts
/// into a single first-class type, replacing the previous loose coupling of
/// `DocContext { &tree, &nav, &reasoning }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Unique document identifier.
    pub doc_id: String,
    /// Document name/title.
    pub name: String,
    /// Document format ("pdf", "markdown", "docx").
    pub format: String,
    /// Source file path (if indexed from a file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,

    // ── Three indexes (engine internal) ──
    /// Hierarchical semantic tree.
    pub tree: super::tree::DocumentTree,
    /// Pre-computed navigation structure.
    pub nav_index: super::navigation::NavigationIndex,
    /// Keyword / topic / section summaries.
    pub reasoning_index: super::reasoning::ReasoningIndex,

    // ── Understanding results (ingest stage output) ──
    /// Document-level summary.
    pub summary: String,
    /// Key concepts the engine identified.
    #[serde(default)]
    pub concepts: Vec<Concept>,

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
    /// Create a read-only agent context from this document.
    ///
    /// Used internally by the retrieval agent for navigation and reasoning.
    pub fn as_context(&self) -> crate::agent::DocContext<'_> {
        crate::agent::DocContext {
            tree: &self.tree,
            nav_index: &self.nav_index,
            reasoning_index: &self.reasoning_index,
            doc_name: &self.name,
        }
    }

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
// Answer — what ask() returns
// ---------------------------------------------------------------------------

/// The result of `ask()` — a reasoned answer with evidence and trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    /// The answer content.
    pub content: String,
    /// Evidence supporting the answer.
    pub evidence: Vec<Evidence>,
    /// Confidence score (0.0–1.0).
    pub confidence: f32,
    /// Reasoning trace — how the agent arrived at this answer. Always present.
    pub trace: ReasoningTrace,
}

// ---------------------------------------------------------------------------
// Evidence
// ---------------------------------------------------------------------------

/// A piece of evidence supporting an answer — with source attribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Original document text.
    pub content: String,
    /// Navigation path (e.g., "Root/Chapter 3/Section 3.2").
    pub source_path: String,
    /// Which document this evidence came from.
    pub doc_name: String,
    /// Relevance to the question (0.0–1.0).
    pub relevance: f32,
}

// ---------------------------------------------------------------------------
// ReasoningTrace — always mandatory
// ---------------------------------------------------------------------------

/// Reasoning trace — how the agent arrived at the answer. Always present.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTrace {
    /// The steps the agent took.
    pub steps: Vec<TraceStep>,
}

impl ReasoningTrace {
    /// Create an empty trace.
    pub fn empty() -> Self {
        Self { steps: Vec::new() }
    }

    /// Create a trace with a single step.
    pub fn single(action: impl Into<String>, observation: impl Into<String>, round: u32) -> Self {
        Self {
            steps: vec![TraceStep {
                action: action.into(),
                observation: observation.into(),
                round,
            }],
        }
    }

    /// Add a step to the trace.
    pub fn push(&mut self, action: impl Into<String>, observation: impl Into<String>, round: u32) {
        self.steps.push(TraceStep {
            action: action.into(),
            observation: observation.into(),
            round,
        });
    }
}

/// A single step in the reasoning trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    /// What the agent did (e.g., "cd Chapter 3").
    pub action: String,
    /// What the agent observed (e.g., "Found 5 sections about...").
    pub observation: String,
    /// Which round this step was in.
    pub round: u32,
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
        format: super::super::index::parse::DocumentFormat,
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
    fn test_reasoning_trace_empty() {
        let trace = ReasoningTrace::empty();
        assert!(trace.steps.is_empty());
    }

    #[test]
    fn test_reasoning_trace_single() {
        let trace = ReasoningTrace::single("cd Chapter 3", "Found 5 sections", 1);
        assert_eq!(trace.steps.len(), 1);
        assert_eq!(trace.steps[0].action, "cd Chapter 3");
        assert_eq!(trace.steps[0].round, 1);
    }

    #[test]
    fn test_reasoning_trace_push() {
        let mut trace = ReasoningTrace::empty();
        trace.push("ls", "Root with 3 children", 0);
        trace.push("cd Chapter 2", "Found target section", 1);
        assert_eq!(trace.steps.len(), 2);
    }

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
