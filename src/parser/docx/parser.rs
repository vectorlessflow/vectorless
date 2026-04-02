// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! DOCX document parser.
//!
//! This module provides functionality to parse DOCX (Microsoft Word) documents:
//! - **DocxParser** — Extract structured content from DOCX files
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::document::docx::DocxParser;
//! use vectorless::core::DocumentParser;
//! use std::path::Path;
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::core::Result<()> {
//! let parser = DocxParser::new();
//! let result = parser.parse_file(Path::new("document.docx")).await?;
//!
//! println!("Extracted {} nodes", result.node_count());
//! for node in &result.nodes {
//!     println!("  - {} (level {})", node.title, node.level);
//! }
//! # Ok(())
//! # }
//! ```

use std::io::{Cursor, Read};
use std::path::Path;

use async_trait::async_trait;
use zip::ZipArchive;

use crate::core::{Error, Result};
use crate::parser::{DocumentFormat, DocumentMeta, DocumentParser, ParseResult, RawNode};

use super::styles::StyleResolver;
use super::types::DocxParagraph;

/// DOCX document parser.
#[derive(Debug, Clone, Default)]
pub struct DocxParser;

impl DocxParser {
    /// Create a new DOCX parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a DOCX file and return raw nodes.
    pub fn parse_file_sync(&self, path: &Path) -> Result<ParseResult> {
        let bytes = std::fs::read(path)
            .map_err(|e| Error::Parse(format!("Failed to read DOCX file: {}", e)))?;

        self.parse_bytes(&bytes, path.file_stem().and_then(|s| s.to_str()))
    }

    /// Parse DOCX from bytes.
    pub fn parse_bytes(&self, bytes: &[u8], filename: Option<&str>) -> Result<ParseResult> {
        // Create ZIP archive from bytes
        let cursor = Cursor::new(bytes);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| Error::Parse(format!("Failed to open DOCX archive: {}", e)))?;

        // Read styles.xml (optional)
        let style_resolver = self.read_styles(&mut archive)?;

        // Read document.xml (required)
        let document_xml = self.read_xml_file(&mut archive, "word/document.xml")?;

        // Parse paragraphs from document
        let paragraphs = self.parse_paragraphs(&document_xml, &style_resolver)?;

        // Convert paragraphs to raw nodes
        let nodes = self.build_raw_nodes(paragraphs)?;

        // Create metadata
        let meta = DocumentMeta {
            name: filename.unwrap_or("Document").to_string(),
            format: DocumentFormat::Docx,
            page_count: None,
            line_count: nodes.len(),
            source_path: None,
            description: None,
        };

        Ok(ParseResult::new(meta, nodes))
    }

    /// Read styles.xml and create a style resolver.
    fn read_styles(&self, archive: &mut ZipArchive<Cursor<&[u8]>>) -> Result<StyleResolver> {
        match self.read_xml_file(archive, "word/styles.xml") {
            Ok(xml) => Ok(StyleResolver::from_xml(&xml)),
            Err(_) => {
                // styles.xml is optional, use default resolver with built-in styles
                Ok(StyleResolver::from_xml(""))
            }
        }
    }

    /// Read an XML file from the archive.
    fn read_xml_file(
        &self,
        archive: &mut ZipArchive<Cursor<&[u8]>>,
        path: &str,
    ) -> Result<String> {
        let mut file = archive
            .by_name(path)
            .map_err(|e| Error::Parse(format!("Failed to read {} from DOCX: {}", path, e)))?;

        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| Error::Parse(format!("Failed to read {} content: {}", path, e)))?;

        Ok(content)
    }

    /// Parse paragraphs from document.xml.
    /// Word namespace URI.
    const WORD_NS: &'static str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    fn parse_paragraphs(
        &self,
        document_xml: &str,
        style_resolver: &StyleResolver,
    ) -> Result<Vec<DocxParagraph>> {
        let doc = roxmltree::Document::parse(document_xml)
            .map_err(|e| Error::Parse(format!("Failed to parse document.xml: {}", e)))?;

        let mut paragraphs = Vec::new();

        // Find all w:p elements (paragraphs)
        for para_elem in doc.descendants().filter(|n| n.has_tag_name((Self::WORD_NS, "p"))) {
            if let Some(para) = self.parse_paragraph(&para_elem, style_resolver) {
                paragraphs.push(para);
            }
        }

        Ok(paragraphs)
    }

    /// Parse a single paragraph element.
    fn parse_paragraph(
        &self,
        elem: &roxmltree::Node,
        style_resolver: &StyleResolver,
    ) -> Option<DocxParagraph> {
        // Extract text from all w:t elements
        let text = self.extract_text(elem);

        if text.trim().is_empty() {
            return None;
        }

        let mut para = DocxParagraph::new(text);

        // Get style from w:pPr/w:pStyle
        for child in elem.children() {
            if child.has_tag_name((Self::WORD_NS, "pPr")) {
                for ppr_child in child.children() {
                    if ppr_child.has_tag_name((Self::WORD_NS, "pStyle")) {
                        if let Some(style_id) = ppr_child.attribute((Self::WORD_NS, "val")) {
                            para.style_id = Some(style_id.to_string());
                            para.heading_level = style_resolver.get_heading_level(&para.style_id);
                        }
                    }
                }
            }
        }

        // If no style found, try heuristics
        if para.heading_level.is_none() {
            para.heading_level = style_resolver.detect_heading_by_heuristics(&para.text);
        }

        Some(para)
    }

    /// Extract text from a paragraph element.
    fn extract_text(&self, elem: &roxmltree::Node) -> String {
        let mut text = String::new();

        // Find all w:t elements (text runs)
        for text_elem in elem.descendants().filter(|n| n.has_tag_name((Self::WORD_NS, "t"))) {
            if let Some(t) = text_elem.text() {
                text.push_str(t);
            }
        }

        text
    }

    /// Build raw nodes from parsed paragraphs.
    fn build_raw_nodes(&self, paragraphs: Vec<DocxParagraph>) -> Result<Vec<RawNode>> {
        let mut nodes: Vec<RawNode> = Vec::new();
        let mut current_sections: Vec<(u8, RawNode)> = Vec::new(); // (level, node)
        let mut has_headings = false;
        let mut unassigned_text: Vec<String> = Vec::new();

        for para in paragraphs {
            if !para.has_content() {
                continue;
            }

            if let Some(level) = para.heading_level {
                // This is a heading - create a new section
                has_headings = true;

                // If there was unassigned text before this heading, add it to the previous section
                if !unassigned_text.is_empty() {
                    if let Some((_, node)) = current_sections.last_mut() {
                        if !node.content.is_empty() {
                            node.content.push('\n');
                        }
                        node.content.push_str(&unassigned_text.join("\n"));
                    }
                    unassigned_text.clear();
                }

                // Save content to previous section at the same or deeper level
                self.finalize_deeper_sections(&mut current_sections, level);

                // Create new section
                let node = RawNode::new(&para.text)
                    .with_level(level as usize);

                current_sections.push((level, node));
            } else {
                // This is body text
                if current_sections.is_empty() {
                    // No sections yet, collect text for later
                    unassigned_text.push(para.text);
                } else {
                    // Append to the deepest current section
                    if let Some((_, node)) = current_sections.last_mut() {
                        if !node.content.is_empty() {
                            node.content.push('\n');
                        }
                        node.content.push_str(&para.text);
                    }
                }
            }
        }

        // Finalize remaining sections
        while let Some((_level, node)) = current_sections.pop() {
            // The tree builder will handle proper hierarchy
            nodes.insert(0, node);
        }

        // If no headings found, create a single node with all content
        if !has_headings {
            let combined = unassigned_text.join("\n");
            let node = RawNode::new("Document")
                .with_content(combined)
                .with_level(1);
            return Ok(vec![node]);
        }

        Ok(nodes)
    }

    /// Finalize sections that are deeper than the given level.
    fn finalize_deeper_sections(
        &self,
        sections: &mut Vec<(u8, RawNode)>,
        new_level: u8,
    ) {
        // Pop sections that are at the same level or deeper
        while let Some((level, _)) = sections.last() {
            if *level >= new_level {
                // This section will be replaced by the new one
                sections.pop();
            } else {
                break;
            }
        }
    }
}

#[async_trait]
impl DocumentParser for DocxParser {
    fn format(&self) -> DocumentFormat {
        DocumentFormat::Docx
    }

    async fn parse(&self, content: &str) -> Result<ParseResult> {
        // For DOCX, content should be a file path
        let path = Path::new(content);
        self.parse_file(path).await
    }

    async fn parse_file(&self, path: &Path) -> Result<ParseResult> {
        // Run sync parsing in a blocking task
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let parser = DocxParser::new();
            parser.parse_file_sync(&path)
        })
        .await
        .map_err(|e| Error::Parse(format!("DOCX parsing task failed: {}", e)))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = DocxParser::new();
        assert_eq!(parser.format(), DocumentFormat::Docx);
    }

    #[test]
    fn test_extract_text() {
        let parser = DocxParser::new();

        // Include namespace declaration for w: prefix
        let xml = r#"
            <w:p xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:r>
                    <w:t>Hello</w:t>
                </w:r>
                <w:r>
                    <w:t> World</w:t>
                </w:r>
            </w:p>
        "#;

        let doc = roxmltree::Document::parse(xml).unwrap();
        let elem = doc.root().first_child().unwrap();
        let text = parser.extract_text(&elem);

        assert_eq!(text, "Hello World");
    }

    #[test]
    fn test_build_raw_nodes_no_headings() {
        let parser = DocxParser::new();

        let paragraphs = vec![
            DocxParagraph::new("First paragraph"),
            DocxParagraph::new("Second paragraph"),
        ];

        let nodes = parser.build_raw_nodes(paragraphs).unwrap();

        assert_eq!(nodes.len(), 1, "Should have exactly one node");
        assert_eq!(nodes[0].title, "Document", "Node title should be 'Document'");
        assert!(
            nodes[0].content.contains("First paragraph"),
            "Content should contain 'First paragraph', got: {:?}",
            nodes[0].content
        );
        assert!(
            nodes[0].content.contains("Second paragraph"),
            "Content should contain 'Second paragraph', got: {:?}",
            nodes[0].content
        );
    }

    #[test]
    fn test_build_raw_nodes_with_headings() {
        let parser = DocxParser::new();

        let mut para1 = DocxParagraph::new("Introduction");
        para1.heading_level = Some(1);

        let para2 = DocxParagraph::new("This is the intro content.");

        let mut para3 = DocxParagraph::new("Details");
        para3.heading_level = Some(2);

        let para4 = DocxParagraph::new("More details here.");

        let paragraphs = vec![para1, para2, para3, para4];

        let nodes = parser.build_raw_nodes(paragraphs).unwrap();

        assert!(nodes.len() >= 2);
        assert!(nodes.iter().any(|n| n.title == "Introduction"));
        assert!(nodes.iter().any(|n| n.title == "Details"));
    }
}
