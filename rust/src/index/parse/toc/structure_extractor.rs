// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Structure extraction from documents without a Table of Contents.
//!
//! When a PDF has no TOC (or all TOC-based extraction modes failed), this
//! module uses LLM to analyse page content and extract the document's
//! hierarchical structure directly.

use tracing::{debug, info, warn};

use crate::config::LlmConfig;
use crate::error::Result;
use crate::index::parse::pdf::PdfPage;

use super::types::TocEntry;
use crate::llm::LlmClient;

/// Configuration for structure extraction.
#[derive(Debug, Clone)]
pub struct StructureExtractorConfig {
    /// Maximum estimated tokens per page group sent to LLM.
    pub max_tokens_per_group: usize,

    /// Number of overlap pages between consecutive groups.
    pub overlap_pages: usize,

    /// LLM configuration.
    pub llm_config: LlmConfig,
}

impl Default for StructureExtractorConfig {
    fn default() -> Self {
        Self {
            max_tokens_per_group: 20_000,
            overlap_pages: 1,
            llm_config: LlmConfig::default(),
        }
    }
}

/// A group of consecutive pages with their combined text.
struct PageGroup {
    /// Combined text with page markers: `<page_N>\n...\n</page_N>`.
    text: String,
    /// Start page number (1-based).
    start_page: usize,
    /// End page number (1-based, inclusive).
    end_page: usize,
}

/// Extracts document structure from page content using LLM.
///
/// Used when a document has no Table of Contents, or when TOC-based extraction
/// failed. Pages are grouped by token count and analysed sequentially: the
/// first group generates an initial structure, subsequent groups append to it.
pub struct StructureExtractor {
    config: StructureExtractorConfig,
    client: LlmClient,
}

impl StructureExtractor {
    /// Create a new structure extractor.
    pub fn new(config: StructureExtractorConfig) -> Self {
        let client = LlmClient::new(config.llm_config.clone().into());
        Self { config, client }
    }

    /// Create an extractor with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(StructureExtractorConfig::default())
    }

    /// Extract hierarchical structure from all pages.
    pub async fn extract(&self, pages: &[PdfPage]) -> Result<Vec<TocEntry>> {
        if pages.is_empty() {
            return Ok(Vec::new());
        }

        let groups = self.group_pages(pages);
        info!(
            "Extracting structure from {} pages in {} groups",
            pages.len(),
            groups.len()
        );

        let mut all_entries = Vec::new();
        let page_count = pages.len();

        for (i, group) in groups.iter().enumerate() {
            let group_entries = if i == 0 {
                self.generate_initial(group).await?
            } else {
                self.generate_continuation(group, &all_entries).await?
            };

            debug!(
                "Group {}/{} (pages {}-{}): extracted {} entries",
                i + 1,
                groups.len(),
                group.start_page,
                group.end_page,
                group_entries.len()
            );

            all_entries.extend(group_entries);
        }

        // Truncate physical_page values that exceed document length
        for entry in &mut all_entries {
            if let Some(p) = entry.physical_page {
                if p > page_count {
                    warn!(
                        "Truncating out-of-range page {} for '{}'",
                        p, entry.title
                    );
                    entry.physical_page = Some(page_count);
                }
            }
        }

        info!("Structure extraction complete: {} entries", all_entries.len());
        Ok(all_entries)
    }

    /// Group pages by estimated token count.
    ///
    /// Each group stays under `max_tokens_per_group`. Consecutive groups
    /// overlap by `overlap_pages` pages to avoid splitting content at
    /// section boundaries.
    fn group_pages(&self, pages: &[PdfPage]) -> Vec<PageGroup> {
        let mut groups = Vec::new();
        let mut group_tokens = 0usize;
        let mut group_pages_buf = Vec::new();

        for (i, page) in pages.iter().enumerate() {
            let new_tokens = group_tokens + page.token_count;

            if new_tokens > self.config.max_tokens_per_group && !group_pages_buf.is_empty() {
                // Finalise current group
                let text = format_group_text(&group_pages_buf);
                groups.push(PageGroup {
                    text,
                    start_page: group_pages_buf.first().unwrap().number,
                    end_page: group_pages_buf.last().unwrap().number,
                });

                // Start new group with overlap
                let overlap_start = i.saturating_sub(self.config.overlap_pages);
                group_pages_buf = pages[overlap_start..=i].to_vec();
                group_tokens = group_pages_buf.iter().map(|p| p.token_count).sum();
            } else {
                group_tokens = new_tokens;
                group_pages_buf.push(page.clone());
            }
        }

        // Final group
        if !group_pages_buf.is_empty() {
            let text = format_group_text(&group_pages_buf);
            groups.push(PageGroup {
                text,
                start_page: group_pages_buf.first().unwrap().number,
                end_page: group_pages_buf.last().unwrap().number,
            });
        }

        groups
    }

    /// Generate initial structure from the first page group.
    async fn generate_initial(&self, group: &PageGroup) -> Result<Vec<TocEntry>> {
        let system = STRUCTURE_EXTRACTION_SYSTEM_PROMPT;
        let user = format!(
            r#"Analyze this document content and extract its hierarchical structure.

Document content:
{}

Return a JSON array:
[
  {{"title": "Section Title", "level": 1, "physical_page": 1}},
  {{"title": "Subsection", "level": 2, "physical_page": 3}},
  ...
]

Rules:
- "level" reflects the hierarchy (1 = chapter/top, 2 = section, 3 = subsection)
- "physical_page" is the page number where the section begins
- Preserve original titles as closely as possible
- Only output the JSON array, no other text"#,
            group.text
        );

        let sections: Vec<ExtractedSection> = self.client.complete_json(system, &user).await?;

        Ok(sections
            .into_iter()
            .map(|s| {
                TocEntry::new(s.title, s.level)
                    .with_physical_page(s.physical_page)
                    .with_confidence(0.7)
            })
            .collect())
    }

    /// Continue structure extraction for a subsequent group.
    ///
    /// Passes previously extracted entries as context so the LLM can
    /// continue the structure rather than restart.
    async fn generate_continuation(
        &self,
        group: &PageGroup,
        previous: &[TocEntry],
    ) -> Result<Vec<TocEntry>> {
        let system = STRUCTURE_EXTRACTION_SYSTEM_PROMPT;

        // Summarise previous entries as context
        let prev_summary = previous
            .iter()
            .rev()
            .take(10)
            .rev()
            .map(|e| {
                format!(
                    "  {{\"title\": \"{}\", \"level\": {}, \"physical_page\": {}}}",
                    e.title,
                    e.level,
                    e.physical_page.unwrap_or(0)
                )
            })
            .collect::<Vec<_>>()
            .join(",\n");

        let user = format!(
            r#"Previously extracted structure:
[
{}
]

Continue extracting structure from these pages:
{}

Return ONLY the NEW entries (do not repeat previous ones):
[
  {{"title": "...", "level": N, "physical_page": M}},
  ...
]

If no new structural elements are found, return: []"#,
            prev_summary, group.text
        );

        let sections: Vec<ExtractedSection> = self.client.complete_json(system, &user).await?;

        Ok(sections
            .into_iter()
            .map(|s| {
                TocEntry::new(s.title, s.level)
                    .with_physical_page(s.physical_page)
                    .with_confidence(0.7)
            })
            .collect())
    }
}

/// Format pages into tagged text for LLM consumption.
fn format_group_text(pages: &[PdfPage]) -> String {
    pages
        .iter()
        .map(|p| {
            // Truncate individual page text if very long
            let text = if p.text.len() > 3000 {
                &p.text[..3000]
            } else {
                &p.text
            };
            format!("<page_{}>\n{}\n</page_{}>", p.number, text, p.number)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

const STRUCTURE_EXTRACTION_SYSTEM_PROMPT: &str = r#"You are a document structure extraction expert. Your task is to analyze document content and extract its hierarchical structure (chapters, sections, subsections).

For each structural element you find, provide:
- title: The section title exactly as it appears
- level: The hierarchy level (1 = chapter/top level, 2 = section, 3 = subsection)
- physical_page: The page number where this section begins

Important:
- Focus on genuine structural elements (chapters, sections), not paragraph topics
- Do NOT include the abstract, summary, or bibliography as structural elements unless they are major sections
- Be conservative: fewer high-quality entries are better than many low-quality ones"#;

/// LLM response type for structure extraction.
#[derive(serde::Deserialize)]
struct ExtractedSection {
    title: String,
    level: usize,
    physical_page: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = StructureExtractorConfig::default();
        assert_eq!(config.max_tokens_per_group, 20_000);
        assert_eq!(config.overlap_pages, 1);
    }

    #[test]
    fn test_group_pages_single_group() {
        let extractor = StructureExtractor::with_defaults();

        let pages: Vec<PdfPage> = (1..=5)
            .map(|i| PdfPage::new(i, format!("Page {} content", i)))
            .collect();

        let groups = extractor.group_pages(&pages);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].start_page, 1);
        assert_eq!(groups[0].end_page, 5);
    }

    #[test]
    fn test_group_pages_multiple_groups() {
        let config = StructureExtractorConfig {
            max_tokens_per_group: 50,
            overlap_pages: 1,
            ..Default::default()
        };
        let extractor = StructureExtractor::new(config);

        // Create pages with enough text to span multiple groups
        let pages: Vec<PdfPage> = (1..=10)
            .map(|i| {
                let text = format!("Page {} content. This is a longer text to use more tokens. ", i).repeat(10);
                PdfPage::new(i, text)
            })
            .collect();

        let groups = extractor.group_pages(&pages);
        assert!(groups.len() > 1, "Expected multiple groups, got {}", groups.len());
    }

    #[test]
    fn test_format_group_text() {
        let pages = vec![
            PdfPage::new(1, "Hello"),
            PdfPage::new(2, "World"),
        ];
        let text = format_group_text(&pages);
        assert!(text.contains("<page_1>"));
        assert!(text.contains("<page_2>"));
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }
}
