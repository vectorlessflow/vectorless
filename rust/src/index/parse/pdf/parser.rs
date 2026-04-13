// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! PDF document parser.
//!
//! Uses [`pdf_extract`] for reliable text extraction (handles CJK, ToUnicode
//! CMap, font encoding, etc.) and [`lopdf`] only for metadata extraction from
//! the PDF Info dictionary.

use std::path::Path;

use lopdf::Document as LopdfDocument;
use tracing::{info, warn};

use crate::Error;
use crate::error::Result;
use crate::index::parse::toc::TocProcessor;
use crate::llm::LlmClient;

use super::types::{PdfMetadata, PdfPage, PdfParseResult};
use crate::index::parse::{DocumentFormat, DocumentMeta, ParseResult, RawNode};

/// PDF document parser.
pub struct PdfParser {
    config: PdfParserConfig,
    /// Optional LLM client for TOC extraction and structure analysis.
    llm_client: Option<LlmClient>,
}

/// PDF parser configuration.
#[derive(Debug, Clone)]
pub struct PdfParserConfig {
    /// Maximum pages to extract (0 = unlimited).
    pub max_pages: usize,

    /// Enable TOC extraction.
    pub extract_toc: bool,
}

impl Default for PdfParserConfig {
    fn default() -> Self {
        Self {
            max_pages: 0,
            extract_toc: true,
        }
    }
}

impl PdfParser {
    /// Create a new PDF parser with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a PDF parser with an externally provided LLM client.
    pub fn with_llm_client(client: LlmClient) -> Self {
        Self {
            config: PdfParserConfig::default(),
            llm_client: Some(client),
        }
    }

    /// Create a parser with custom configuration.
    pub fn with_config(config: PdfParserConfig) -> Self {
        Self {
            config,
            llm_client: None,
        }
    }

    /// Create a parser without TOC extraction.
    pub fn without_toc() -> Self {
        Self {
            config: PdfParserConfig {
                extract_toc: false,
                ..Default::default()
            },
            llm_client: None,
        }
    }

    /// Parse PDF from bytes and return raw pages.
    pub async fn parse_bytes_raw(
        &self,
        bytes: &[u8],
        filename: Option<&str>,
    ) -> Result<PdfParseResult> {
        // Use pdf-extract for text (handles CJK, ToUnicode CMap, etc.)
        let pages = self.extract_pages(bytes)?;

        // Use lopdf only for metadata; fall back gracefully if it fails
        let metadata = match LopdfDocument::load_mem(bytes) {
            Ok(doc) => self.extract_metadata(&doc, filename),
            Err(_) => PdfMetadata {
                title: filename.unwrap_or("Document").to_string(),
                page_count: pages.len(),
                ..Default::default()
            },
        };

        Ok(PdfParseResult::new(metadata, pages))
    }

    /// Extract text from all pages using pdf-extract.
    fn extract_pages(&self, bytes: &[u8]) -> Result<Vec<PdfPage>> {
        let page_texts = pdf_extract::extract_text_from_mem_by_pages(bytes)
            .map_err(|e| Error::Parse(format!("pdf-extract failed: {}", e)))?;

        let mut pages = Vec::new();
        for (i, text) in page_texts.iter().enumerate() {
            if self.config.max_pages > 0 && i >= self.config.max_pages {
                break;
            }
            let page_num = i + 1; // 1-based
            if !text.trim().is_empty() {
                pages.push(PdfPage::new(page_num, text.clone()));
            }
        }

        Ok(pages)
    }

    /// Extract metadata from PDF Info dictionary via lopdf.
    fn extract_metadata(&self, doc: &LopdfDocument, filename: Option<&str>) -> PdfMetadata {
        let mut metadata = PdfMetadata {
            title: filename.unwrap_or("Document").to_string(),
            page_count: doc.get_pages().len(),
            ..Default::default()
        };

        if let Ok(info) = doc.trailer.get(b"Info") {
            if let Ok(info_ref) = info.as_reference() {
                if let Ok(info_obj) = doc.get_object(info_ref) {
                    if let Ok(dict) = info_obj.as_dict() {
                        if let Ok(title_obj) = dict.get(b"Title") {
                            if let Ok(title) = title_obj.as_str() {
                                metadata.title = self.decode_pdf_string(title);
                            }
                        }

                        if let Ok(author_obj) = dict.get(b"Author") {
                            if let Ok(author) = author_obj.as_str() {
                                metadata.author = Some(self.decode_pdf_string(author));
                            }
                        }

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

    /// Decode PDF string literal (handles escape sequences).
    ///
    /// Used only for metadata field values extracted via lopdf.
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

    /// Convert TOC entries to RawNodes.
    fn toc_entries_to_raw_nodes(
        &self,
        entries: &[crate::index::parse::toc::TocEntry],
        pages: &[PdfPage],
    ) -> Vec<RawNode> {
        let mut nodes = Vec::new();

        for entry in entries {
            let content = self.get_content_for_entry(entry, pages);

            let mut node = RawNode::new(&entry.title)
                .with_content(content)
                .with_level(entry.level);

            if let Some(page) = entry.physical_page {
                node = node.with_page(page);
            }

            nodes.push(node);
        }

        nodes
    }

    /// Get content for a TOC entry from pages.
    fn get_content_for_entry(
        &self,
        entry: &crate::index::parse::toc::TocEntry,
        pages: &[PdfPage],
    ) -> String {
        let start_page = entry.physical_page.unwrap_or(1);

        pages
            .iter()
            .find(|p| p.number == start_page)
            .map(|p| {
                let text = &p.text;
                if let Some(pos) = text.find(&entry.title) {
                    text[pos + entry.title.len()..].trim().to_string()
                } else {
                    text.clone()
                }
            })
            .unwrap_or_default()
    }

    /// Create RawNodes from pages (fallback when no TOC).
    fn pages_to_raw_nodes(&self, pages: &[PdfPage]) -> Vec<RawNode> {
        pages
            .iter()
            .map(|page| {
                RawNode::new(format!("Page {}", page.number))
                    .with_content(page.text.clone())
                    .with_level(1)
                    .with_page(page.number)
            })
            .collect()
    }
}

impl Default for PdfParser {
    fn default() -> Self {
        Self::with_config(PdfParserConfig::default())
    }
}

impl PdfParser {
    /// Parse a PDF file into raw nodes for the index pipeline.
    pub async fn parse_file(&self, path: &Path) -> Result<ParseResult> {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| Error::Parse(format!("Failed to read PDF file: {}", e)))?;
        let filename = path.file_stem().and_then(|s| s.to_str());
        self.parse_bytes_to_result(&bytes, filename, Some(path))
            .await
    }

    /// Parse PDF bytes into raw nodes for the index pipeline.
    pub async fn parse_bytes_async(
        &self,
        bytes: &[u8],
        filename: Option<&str>,
    ) -> Result<ParseResult> {
        self.parse_bytes_to_result(bytes, filename, None).await
    }

    /// Core async parsing logic shared by parse_file and parse_bytes_async.
    async fn parse_bytes_to_result(
        &self,
        bytes: &[u8],
        filename: Option<&str>,
        source_path: Option<&Path>,
    ) -> Result<ParseResult> {
        let result = self.parse_bytes_raw(bytes, filename).await?;
        let page_count = result.pages.len();

        // Try TOC extraction if enabled
        let nodes = if self.config.extract_toc {
            info!("Extracting TOC from PDF with {} pages", page_count);

            let processor = match &self.llm_client {
                Some(client) => {
                    info!("PdfParser: creating TocProcessor with LLM client");
                    TocProcessor::with_llm_client(client.clone())
                }
                None => {
                    info!("PdfParser: creating TocProcessor without LLM client (no key configured)");
                    TocProcessor::new()
                }
            };
            match processor.process(&result.pages).await {
                Ok(entries) if !entries.is_empty() => {
                    info!("Extracted {} TOC entries", entries.len());
                    self.toc_entries_to_raw_nodes(&entries, &result.pages)
                }
                Ok(_) => {
                    warn!("No TOC entries found, falling back to page-based extraction");
                    self.pages_to_raw_nodes(&result.pages)
                }
                Err(e) => {
                    warn!(
                        "TOC extraction failed: {}, falling back to page-based extraction",
                        e
                    );
                    self.pages_to_raw_nodes(&result.pages)
                }
            }
        } else {
            self.pages_to_raw_nodes(&result.pages)
        };

        let meta = DocumentMeta {
            name: result.metadata.title,
            format: DocumentFormat::Pdf,
            page_count: Some(page_count),
            line_count: 0,
            source_path: source_path.map(|p| p.to_string_lossy().to_string()),
            description: result.metadata.subject,
        };

        Ok(ParseResult::new(meta, nodes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = PdfParser::new();
        assert_eq!(parser.config.max_pages, 0);
        assert!(parser.config.extract_toc);
    }

    #[test]
    fn test_parser_without_toc() {
        let parser = PdfParser::without_toc();
        assert!(!parser.config.extract_toc);
    }

    #[test]
    fn test_decode_pdf_string() {
        let parser = PdfParser::new();

        let decoded = parser.decode_pdf_string(b"Hello World");
        assert_eq!(decoded, "Hello World");

        let decoded = parser.decode_pdf_string(b"Hello\\nWorld");
        assert_eq!(decoded, "Hello\nWorld");
    }
}
