// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! TOC (Table of Contents) detection.

use regex::Regex;
use tracing::debug;

use crate::config::LlmConfig;
use crate::core::Result;

use super::llm_client::LlmClient;
use super::types::TocDetection;
use crate::document::pdf::PdfPage;

/// TOC detector configuration.
#[derive(Debug, Clone)]
pub struct TocDetectorConfig {
    /// Maximum pages to check for TOC.
    pub max_check_pages: usize,

    /// Minimum confidence threshold for regex detection.
    pub regex_confidence_threshold: f32,

    /// Use LLM for uncertain cases.
    pub use_llm_fallback: bool,

    /// LLM configuration.
    pub llm_config: LlmConfig,
}

impl Default for TocDetectorConfig {
    fn default() -> Self {
        Self {
            max_check_pages: 15,
            regex_confidence_threshold: 0.7,
            use_llm_fallback: true,
            llm_config: LlmConfig::default(),
        }
    }
}

/// TOC detector - finds table of contents in PDF documents.
pub struct TocDetector {
    config: TocDetectorConfig,
    llm_client: Option<LlmClient>,
    patterns: Vec<TocPattern>,
}

/// A TOC detection pattern.
struct TocPattern {
    name: &'static str,
    regex: Regex,
    weight: f32,
}

impl TocDetector {
    /// Create a new TOC detector.
    pub fn new(config: TocDetectorConfig) -> Self {
        let llm_client = if config.use_llm_fallback {
            Some(LlmClient::new(config.llm_config.clone()))
        } else {
            None
        };

        Self {
            config,
            llm_client,
            patterns: Self::build_patterns(),
        }
    }

    /// Create a detector with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(TocDetectorConfig::default())
    }

    /// Build detection patterns.
    fn build_patterns() -> Vec<TocPattern> {
        vec![
            // Chinese TOC patterns
            TocPattern {
                name: "chinese_toc_header",
                regex: Regex::new(r"(?i)^[\s]*(目\s*录|内\s*容\s*摘\s*要)[\s]*$").unwrap(),
                weight: 0.9,
            },
            TocPattern {
                name: "chinese_chapter_with_page",
                regex: Regex::new(r"第[一二三四五六七八九十\d]+[章节部篇].*?[\.\s…·]{2,}\s*\d+").unwrap(),
                weight: 0.85,
            },
            TocPattern {
                name: "chinese_section_dots",
                regex: Regex::new(r"\d+[\.\d]+\s+.+?\s*[\.\s…·]{3,}\s*\d+").unwrap(),
                weight: 0.8,
            },
            // English TOC patterns
            TocPattern {
                name: "english_toc_header",
                regex: Regex::new(r"(?i)^[\s]*(table\s+of\s+contents|contents|outline)[\s]*$").unwrap(),
                weight: 0.9,
            },
            TocPattern {
                name: "english_chapter_with_page",
                regex: Regex::new(r"(?i)^[\s]*(chapter|section|part)\s+\d+.*?\d+\s*$").unwrap(),
                weight: 0.85,
            },
            TocPattern {
                name: "numbered_section_dots",
                regex: Regex::new(r"^\d+\.\d+(\.\d+)?\s+.+?[\.\s…]{3,}\s*\d+\s*$").unwrap(),
                weight: 0.75,
            },
            // Generic patterns
            TocPattern {
                name: "dots_leader",
                regex: Regex::new(r".+?[\.\s…·]{4,}\s*\d{1,4}\s*$").unwrap(),
                weight: 0.7,
            },
            TocPattern {
                name: "title_with_page",
                regex: Regex::new(r"^.{3,50}?\s{2,}\d{1,4}\s*$").unwrap(),
                weight: 0.5,
            },
        ]
    }

    /// Detect TOC in PDF pages.
    pub async fn detect(&self, pages: &[PdfPage]) -> Result<TocDetection> {
        let check_pages = pages.iter()
            .take(self.config.max_check_pages)
            .collect::<Vec<_>>();

        if check_pages.is_empty() {
            return Ok(TocDetection::not_found());
        }

        // Step 1: Regex detection
        let regex_result = self.detect_with_regex(&check_pages);
        debug!("Regex detection result: found={}, confidence={}",
            regex_result.found, regex_result.confidence);

        // Step 2: If confidence is high enough, return
        if regex_result.confidence >= self.config.regex_confidence_threshold {
            return Ok(regex_result);
        }

        // Step 3: Use LLM fallback if available and needed
        if let Some(ref client) = self.llm_client {
            if regex_result.confidence > 0.3 || regex_result.confidence == 0.0 {
                debug!("Using LLM fallback for TOC detection");
                return self.detect_with_llm(client, &check_pages).await;
            }
        }

        Ok(regex_result)
    }

    /// Detect TOC using regex patterns.
    fn detect_with_regex(&self, pages: &[&PdfPage]) -> TocDetection {
        let mut toc_pages = Vec::new();
        let mut has_page_numbers = false;
        let mut total_score = 0.0;
        let mut match_count = 0;

        for page in pages {
            let (score, has_numbers) = self.score_page_for_toc(page);

            if score > 0.5 {
                toc_pages.push(page.number);

                if has_numbers {
                    has_page_numbers = true;
                }

                total_score += score;
                match_count += 1;
            }
        }

        if toc_pages.is_empty() {
            return TocDetection::not_found();
        }

        let confidence = if match_count > 0 {
            total_score / match_count as f32
        } else {
            0.0
        };

        TocDetection::new(true)
            .with_pages(toc_pages)
            .with_page_numbers(has_page_numbers)
            .with_confidence(confidence)
    }

    /// Score a page for TOC likelihood.
    fn score_page_for_toc(&self, page: &PdfPage) -> (f32, bool) {
        let lines: Vec<&str> = page.text.lines().collect();

        if lines.len() < 2 {
            return (0.0, false);
        }

        let mut max_score: f32 = 0.0;
        let mut has_page_numbers = false;
        let mut match_count = 0;

        for line in &lines {
            for pattern in &self.patterns {
                if pattern.regex.is_match(line) {
                    max_score = max_score.max(pattern.weight);
                    match_count += 1;

                    // Check if pattern includes page numbers
                    if line.matches(char::is_numeric).count() > 0 {
                        has_page_numbers = true;
                    }
                }
            }
        }

        // Adjust score based on number of matches
        let score = if match_count >= 3 {
            max_score
        } else if match_count >= 1 {
            max_score * 0.7
        } else {
            0.0
        };

        (score, has_page_numbers)
    }

    /// Detect TOC using LLM.
    async fn detect_with_llm(&self, client: &LlmClient, pages: &[&PdfPage]) -> Result<TocDetection> {
        // Combine first few pages for analysis
        let content = pages.iter()
            .take(5)
            .map(|p| format!("<page_{}>\n{}\n</page_{}>", p.number,
                &p.text[..p.text.len().min(1000)], p.number))
            .collect::<Vec<_>>()
            .join("\n\n");

        let system = "You are a document analysis assistant. Your task is to detect if the given document contains a Table of Contents (TOC).";
        let user = format!(
            r#"Analyze this document and determine if it contains a Table of Contents.

Document content:
{}

Reply in JSON format:
{{
    "has_toc": true/false,
    "toc_pages": [list of page numbers where TOC appears],
    "has_page_numbers": true/false (whether TOC entries include page numbers),
    "confidence": 0.0-1.0
}}"#,
            content
        );

        #[derive(serde::Deserialize)]
        struct DetectionResponse {
            has_toc: bool,
            toc_pages: Vec<usize>,
            has_page_numbers: bool,
            confidence: f32,
        }

        let response: DetectionResponse = client.complete_json(system, &user).await?;

        Ok(TocDetection::new(response.has_toc)
            .with_pages(response.toc_pages)
            .with_page_numbers(response.has_page_numbers)
            .with_confidence(response.confidence))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_page(number: usize, text: &str) -> PdfPage {
        PdfPage::new(number, text)
    }

    #[test]
    fn test_detect_chinese_toc() {
        let detector = TocDetector::with_defaults();

        let pages = vec![
            make_page(1, "前言"),
            make_page(2, "目  录\n\n第一章 引言 ... 1\n第二章 方法 ... 5"),
        ];

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(detector.detect(&pages)).unwrap();

        assert!(result.found);
        assert!(result.has_page_numbers);
    }

    #[test]
    fn test_detect_english_toc() {
        let detector = TocDetector::with_defaults();

        let pages = vec![
            make_page(1, "Abstract"),
            make_page(2, "Table of Contents\n\nChapter 1. Introduction  1\nChapter 2. Methods  5"),
        ];

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(detector.detect(&pages)).unwrap();

        assert!(result.found);
    }

    #[test]
    fn test_no_toc() {
        let detector = TocDetector::with_defaults();

        let pages = vec![
            make_page(1, "This is a simple document."),
            make_page(2, "It has no table of contents."),
        ];

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(detector.detect(&pages)).unwrap();

        assert!(!result.found);
    }
}
