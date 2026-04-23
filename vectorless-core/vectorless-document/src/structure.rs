// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document structure types for JSON export.
//!

use serde::{Deserialize, Serialize};

/// A node in the document structure for JSON export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureNode {
    /// Node title.
    pub title: String,
    /// Unique node identifier.
    pub node_id: String,
    /// Starting line number (1-based).
    pub start_index: usize,
    /// Ending line number (1-based).
    pub end_index: usize,
    /// Generated summary (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Child nodes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<StructureNode>,
}

/// Document structure for JSON export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStructure {
    /// Document name.
    pub doc_name: String,
    /// Tree structure.
    pub structure: Vec<StructureNode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structure_node_serialization() {
        let node = StructureNode {
            title: "Introduction".to_string(),
            node_id: "0001".to_string(),
            start_index: 1,
            end_index: 10,
            summary: Some("A brief intro".to_string()),
            nodes: vec![],
        };

        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("Introduction"));
    }

    #[test]
    fn test_document_structure() {
        let doc = DocumentStructure {
            doc_name: "test.md".to_string(),
            structure: vec![],
        };

        assert_eq!(doc.doc_name, "test.md");
    }
}
