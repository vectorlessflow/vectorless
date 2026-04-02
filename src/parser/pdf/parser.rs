// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! PDF document parser using lopdf.

use std::path::Path;

use lopdf::Document as LopdfDocument;

use crate::core::{Error, Result};
use crate::parser::DocumentParser;

use super::types::{PdfMetadata, PdfPage, PdfParseResult};
use crate::parser::{DocumentFormat, DocumentMeta, ParseResult, RawNode};

/// PDF document parser.
#[derive(Debug, Clone, Default)]
pub struct PdfParser {
    config: PdfParserConfig,
}

/// PDF parser configuration.
#[derive(Debug, Clone)]
pub struct PdfParserConfig {
    /// Maximum pages to extract (0 = unlimited).
    pub max_pages: usize,
}

impl Default for PdfParserConfig {
    fn default() -> Self {
        Self { max_pages: 0 }
    }
}

impl PdfParser {
    /// Create a new PDF parser with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a parser with custom configuration.
    pub fn with_config(config: PdfParserConfig) -> Self {
        Self { config }
    }

    /// Parse a PDF file and return detailed result.
    pub fn parse_file(&self, path: &Path) -> Result<PdfParseResult> {
        let bytes = std::fs::read(path)
            .map_err(|e| Error::Parse(format!("Failed to read PDF file: {}", e)))?;

        self.parse_bytes(&bytes, path.file_stem().and_then(|s| s.to_str()))
    }

    /// Parse PDF from bytes.
    pub fn parse_bytes(&self, bytes: &[u8], filename: Option<&str>) -> Result<PdfParseResult> {
        let doc = LopdfDocument::load_mem(bytes)
            .map_err(|e| Error::Parse(format!("Failed to parse PDF: {}", e)))?;

        // Extract metadata
        let metadata = self.extract_metadata(&doc, filename);

        // Extract pages
        let pages = self.extract_pages(&doc)?;

        Ok(PdfParseResult::new(metadata, pages))
    }

    /// Extract metadata from PDF document.
    fn extract_metadata(&self, doc: &LopdfDocument, filename: Option<&str>) -> PdfMetadata {
        let mut metadata = PdfMetadata {
            title: filename.unwrap_or("Document").to_string(),
            page_count: doc.get_pages().len(),
            ..Default::default()
        };

        // Try to extract metadata from Info dictionary
        if let Ok(info) = doc.trailer.get(b"Info") {
            if let Ok(info_ref) = info.as_reference() {
                if let Ok(info_obj) = doc.get_object(info_ref) {
                    if let Ok(dict) = info_obj.as_dict() {
                        // Title
                        if let Ok(title_obj) = dict.get(b"Title") {
                            if let Ok(title) = title_obj.as_str() {
                                metadata.title = self.decode_pdf_string(title);
                            }
                        }

                        // Author
                        if let Ok(author_obj) = dict.get(b"Author") {
                            if let Ok(author) = author_obj.as_str() {
                                metadata.author = Some(self.decode_pdf_string(author));
                            }
                        }

                        // Subject
                        if let Ok(subject_obj) = dict.get(b"Subject") {
                            if let Ok(subject) = subject_obj.as_str() {
                                metadata.subject = Some(self.decode_pdf_string(subject));
                            }
                        }
                    }
                }
            }
        }

        metadata
    }

    /// Extract text from all pages.
    fn extract_pages(&self, doc: &LopdfDocument) -> Result<Vec<PdfPage>> {
        let page_map = doc.get_pages();
        let mut pages = Vec::new();

        for (i, (page_num, object_id)) in page_map.iter().enumerate() {
            // Check max pages limit
            if self.config.max_pages > 0 && i >= self.config.max_pages {
                break;
            }

            let text = self.extract_page_text(doc, *object_id, *page_num as usize);

            // Skip empty pages
            if !text.trim().is_empty() {
                pages.push(PdfPage::new(*page_num as usize, text));
            }
        }

        Ok(pages)
    }

    /// Extract text from a single page.
    fn extract_page_text(&self, doc: &LopdfDocument, object_id: lopdf::ObjectId, _page_num: usize) -> String {
        let mut text = String::new();

        if let Ok(page_obj) = doc.get_object(object_id) {
            if let Ok(page_dict) = page_obj.as_dict() {
                if let Ok(contents) = page_dict.get(b"Contents") {
                    match contents {
                        lopdf::Object::Reference(ref_id) => {
                            if let Ok(content_obj) = doc.get_object(*ref_id) {
                                if let Ok(stream) = content_obj.as_stream() {
                                    text = self.decode_stream_content(stream);
                                }
                            }
                        }
                        lopdf::Object::Array(arr) => {
                            for obj in arr {
                                if let Ok(ref_id) = obj.as_reference() {
                                    if let Ok(content_obj) = doc.get_object(ref_id) {
                                        if let Ok(stream) = content_obj.as_stream() {
                                            let content = self.decode_stream_content(stream);
                                            if !text.is_empty() {
                                                text.push('\n');
                                            }
                                            text.push_str(&content);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Post-process text
        self.post_process_text(&text)
    }

    /// Decode stream content to text.
    fn decode_stream_content(&self, stream: &lopdf::Stream) -> String {
        // Try to decode the stream
        if let Ok(content) = stream.decompressed_content() {
            self.extract_text_from_content(&content)
        } else {
            self.extract_text_from_content(&stream.content)
        }
    }

    /// Extract text from PDF content stream (simplified).
    fn extract_text_from_content(&self, content: &[u8]) -> String {
        let content_str = String::from_utf8_lossy(content);
        let mut text = String::new();

        for line in content_str.lines() {
            let line = line.trim();

            // Tj operator: (text) Tj
            if line.ends_with("Tj") {
                if let Some(text_part) = self.extract_parentheses_text(line) {
                    text.push_str(&text_part);
                }
            }
            // TJ operator: [(text) ...] TJ
            else if line.ends_with("TJ") {
                if let Some(text_parts) = self.extract_array_text(line) {
                    text.push_str(&text_parts);
                }
            }
        }

        text
    }

    /// Extract text from parentheses in Tj operator.
    fn extract_parentheses_text(&self, line: &str) -> Option<String> {
        let start = line.find('(')?;
        let end = line.rfind(')')?;
        if end > start {
            let raw = &line[start + 1..end];
            Some(self.decode_pdf_string(raw.as_bytes()))
        } else {
            None
        }
    }

    /// Extract text from array in TJ operator.
    fn extract_array_text(&self, line: &str) -> Option<String> {
        let start = line.find('[')?;
        let end = line.rfind(']')?;
        if end > start {
            let content = &line[start + 1..end];
            let mut text = String::new();

            let mut in_parens = false;
            let mut current = String::new();

            for ch in content.chars() {
                match ch {
                    '(' => {
                        in_parens = true;
                        current.clear();
                    }
                    ')' => {
                        if in_parens {
                            text.push_str(&self.decode_pdf_string(current.as_bytes()));
                        }
                        in_parens = false;
                    }
                    _ => {
                        if in_parens {
                            current.push(ch);
                        }
                    }
                }
            }

            Some(text)
        } else {
            None
        }
    }

    /// Decode PDF string (handle escape sequences).
    fn decode_pdf_string(&self, bytes: &[u8]) -> String {
        let mut result = String::new();
        let mut i = 0;

        while i < bytes.len() {
            match bytes[i] {
                b'\\' if i + 1 < bytes.len() => {
                    i += 1;
                    match bytes[i] {
                        b'n' => result.push('\n'),
                        b'r' => result.push('\r'),
                        b't' => result.push('\t'),
                        b'(' => result.push('('),
                        b')' => result.push(')'),
                        b'\\' => result.push('\\'),
                        _ => {}
                    }
                }
                b if b >= 32 && b < 127 => {
                    result.push(b as char);
                }
                _ => {}
            }
            i += 1;
        }

        result
    }

    /// Post-process extracted text.
    fn post_process_text(&self, text: &str) -> String {
        let mut result = String::new();
        let mut prev_space = false;

        for ch in text.chars() {
            if ch.is_whitespace() {
                if !prev_space {
                    result.push(' ');
                    prev_space = true;
                }
            } else {
                result.push(ch);
                prev_space = false;
            }
        }

        result.trim().to_string()
    }
}

#[async_trait::async_trait]
impl DocumentParser for PdfParser {
    fn format(&self) -> DocumentFormat {
        DocumentFormat::Pdf
    }

    async fn parse(&self, content: &str) -> Result<ParseResult> {
        // For PDF, content is the file path
        let path = Path::new(content);
        self.parse_path(path).await
    }

    async fn parse_file(&self, path: &Path) -> Result<ParseResult> {
        self.parse_path(path).await
    }
}

impl PdfParser {
    /// Internal async method to parse PDF from path.
    async fn parse_path(&self, path: &Path) -> Result<ParseResult> {
        let result = self.parse_pdf_file(path)?;

        // Convert PdfParseResult to ParseResult
        let meta = DocumentMeta {
            name: result.metadata.title,
            format: DocumentFormat::Pdf,
            page_count: Some(result.pages.len()),
            line_count: 0,
            source_path: Some(path.to_string_lossy().to_string()),
            description: result.metadata.subject,
        };

        // Create raw nodes from pages
        let nodes: Vec<RawNode> = result.pages
            .iter()
            .map(|page| {
                RawNode::new(&format!("Page {}", page.number))
                    .with_content(page.text.clone())
                    .with_level(1)
                    .with_page(page.number)
            })
            .collect();

        Ok(ParseResult::new(meta, nodes))
    }

    /// Parse a PDF file and return detailed result.
    fn parse_pdf_file(&self, path: &Path) -> Result<PdfParseResult> {
        let bytes = std::fs::read(path)
            .map_err(|e| Error::Parse(format!("Failed to read PDF file: {}", e)))?;

        self.parse_bytes(&bytes, path.file_stem().and_then(|s| s.to_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = PdfParser::new();
        assert_eq!(parser.config.max_pages, 0);
    }

    #[test]
    fn test_decode_pdf_string() {
        let parser = PdfParser::new();

        let decoded = parser.decode_pdf_string(b"Hello World");
        assert_eq!(decoded, "Hello World");

        let decoded = parser.decode_pdf_string(b"Hello\\nWorld");
        assert_eq!(decoded, "Hello\nWorld");
    }

    #[test]
    fn test_post_process_text() {
        let parser = PdfParser::new();

        let processed = parser.post_process_text("Hello   World");
        assert_eq!(processed, "Hello World");

        let processed = parser.post_process_text("  Hello  World  ");
        assert_eq!(processed, "Hello World");
    }
}
