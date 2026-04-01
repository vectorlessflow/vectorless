// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Page assigner - assigns physical page numbers to TOC entries.

use std::collections::HashMap;
use tracing::{debug, info};

use crate::config::LlmConfig;
use crate::core::Result;
use crate::document::pdf::PdfPage;

use super::llm_client::LlmClient;
use super::types::{PageOffset, TocEntry};

/// Page assigner configuration.
#[derive(Debug, Clone)]
pub struct PageAssignerConfig {
    /// Number of anchor points for offset calculation.
    pub anchor_count: usize,

    /// LLM configuration.
    pub llm_config: LlmConfig,

    /// Maximum offset variance allowed.
    pub max_offset_variance: usize,
}

impl Default for PageAssignerConfig {
    fn default() -> Self {
        Self {
            anchor_count: 5,
            llm_config: LlmConfig::default(),
            max_offset_variance: 3,
        }
    }
}

/// Page assigner - assigns physical page numbers to TOC entries.
pub struct PageAssigner {
    config: PageAssignerConfig,
    client: LlmClient,
}

impl PageAssigner {
    /// Create a new page assigner.
    pub fn new(config: PageAssignerConfig) -> Self {
        let client = LlmClient::new(config.llm_config.clone());
        Self { config, client }
    }

    /// Create an assigner with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(PageAssignerConfig::default())
    }

    /// Assign physical pages to TOC entries.
    ///
    /// Strategy:
    /// 1. If entries have TOC pages → calculate offset → apply offset
    /// 2. If no TOC pages → use LLM to locate each entry
    pub async fn assign(&self, entries: &mut [TocEntry], pages: &[PdfPage]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        // Check if we have TOC page numbers
        let has_toc_pages = entries.iter().any(|e| e.toc_page.is_some());

        if has_toc_pages {
            self.assign_with_offset(entries, pages).await
        } else {
            self.assign_with_llm(entries, pages).await
        }
    }

    /// Assign pages using offset calculation.
    async fn assign_with_offset(&self, entries: &mut [TocEntry], pages: &[PdfPage]) -> Result<()> {
        info!("Assigning pages using offset calculation");

        // Step 1: Select anchor entries
        let anchors = self.select_anchors(entries, self.config.anchor_count);

        // Step 2: Verify anchors and calculate offset
        let offset = self.calculate_offset(anchors, pages).await?;

        if offset.confidence < 0.5 {
            debug!("Offset confidence too low, falling back to LLM positioning");
            return self.assign_with_llm(entries, pages).await;
        }

        info!("Calculated offset: {} (confidence: {})", offset.offset, offset.confidence);

        // Step 3: Apply offset to all entries
        for entry in entries.iter_mut() {
            if let Some(toc_page) = entry.toc_page {
                let physical = offset.apply(toc_page);
                entry.physical_page = Some(physical.min(pages.len()));
            }
        }

        Ok(())
    }

    /// Select anchor entries for offset calculation.
    fn select_anchors<'a>(&self, entries: &'a [TocEntry], count: usize) -> Vec<&'a TocEntry> {
        // Select entries with TOC pages, evenly distributed
        let with_pages: Vec<_> = entries.iter()
            .filter(|e| e.toc_page.is_some())
            .collect();

        if with_pages.len() <= count {
            return with_pages;
        }

        // Select evenly distributed entries
        let step = with_pages.len() as f32 / count as f32;
        (0..count)
            .map(|i| with_pages[(i as f32 * step) as usize])
            .collect()
    }

    /// Calculate page offset by verifying anchors.
    async fn calculate_offset(&self, anchors: Vec<&TocEntry>, pages: &[PdfPage]) -> Result<PageOffset> {
        if anchors.is_empty() {
            return Ok(PageOffset::new(0, 0, 0.0));
        }

        let anchor_count = anchors.len();
        let mut verified_offsets: Vec<(i32, bool)> = Vec::new();

        for anchor in anchors {
            let toc_page = anchor.toc_page.unwrap();

            // Find the physical page where this title appears
            if let Some(physical) = self.locate_title_in_range(anchor.title.as_str(), pages, toc_page).await? {
                let offset = physical as i32 - toc_page as i32;
                verified_offsets.push((offset, true));
                debug!("Anchor '{}' found: toc={}, physical={}, offset={}",
                    anchor.title, toc_page, physical, offset);
            } else {
                verified_offsets.push((0, false));
            }
        }

        // Calculate the mode (most common offset)
        let successful: Vec<_> = verified_offsets.iter()
            .filter(|(_, success)| *success)
            .map(|(offset, _)| *offset)
            .collect();

        if successful.is_empty() {
            return Ok(PageOffset::new(0, 0, 0.0));
        }

        let mode = self.calculate_mode(&successful);
        let sample_count = successful.len();
        let confidence = sample_count as f32 / anchor_count as f32;

        Ok(PageOffset::new(mode, sample_count, confidence))
    }

    /// Calculate mode of offset values.
    fn calculate_mode(&self, values: &[i32]) -> i32 {
        let mut counts: HashMap<i32, usize> = HashMap::new();
        for &v in values {
            *counts.entry(v).or_insert(0) += 1;
        }
        counts.into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(v, _)| v)
            .unwrap_or(0)
    }

    /// Locate a title in a range of pages using LLM.
    async fn locate_title_in_range(&self, title: &str, pages: &[PdfPage], near_page: usize) -> Result<Option<usize>> {
        // Search in a range around the expected page
        let start = (near_page.saturating_sub(3)).max(1);
        let end = (near_page + 3).min(pages.len());

        let range_pages: Vec<_> = (start..=end)
            .filter_map(|i| pages.get(i - 1))
            .collect();

        if range_pages.is_empty() {
            return Ok(None);
        }

        // Use LLM to find the exact page
        let content = range_pages.iter()
            .map(|p| format!("<page_{}>\n{}\n</page_{}>",
                p.number,
                &p.text[..p.text.len().min(500)],
                p.number))
            .collect::<Vec<_>>()
            .join("\n\n");

        let system = "You are a document analysis assistant. Find which page contains a specific section title.";
        let user = format!(
            r#"Find which page contains the section titled: "{}"

Pages:
{}

Reply in JSON format:
{{"page": <page_number or null>}}"#,
            title, content
        );

        #[derive(serde::Deserialize)]
        struct LocateResult {
            page: Option<usize>,
        }

        let result: LocateResult = self.client.complete_json(system, &user).await?;
        Ok(result.page)
    }

    /// Assign pages using LLM for each entry.
    async fn assign_with_llm(&self, entries: &mut [TocEntry], pages: &[PdfPage]) -> Result<()> {
        info!("Assigning pages using LLM positioning");

        // Group pages for efficient processing
        let page_groups = self.group_pages(pages, 5);

        for entry in entries.iter_mut() {
            let physical = self.locate_title_in_groups(entry.title.as_str(), &page_groups).await?;
            entry.physical_page = physical;
            entry.confidence = if physical.is_some() { 0.8 } else { 0.3 };
        }

        Ok(())
    }

    /// Group pages for batch processing.
    fn group_pages<'a>(&self, pages: &'a [PdfPage], group_size: usize) -> Vec<Vec<&'a PdfPage>> {
        pages.chunks(group_size)
            .map(|chunk| chunk.iter().collect())
            .collect()
    }

    /// Locate a title across page groups.
    async fn locate_title_in_groups(&self, title: &str, groups: &[Vec<&PdfPage>]) -> Result<Option<usize>> {
        let system = "You are a document analysis assistant. Find which page contains a specific section title.";

        for group in groups {
            let content = group.iter()
                .map(|p| format!("<page_{}>\n{}\n</page_{}>",
                    p.number,
                    &p.text[..p.text.len().min(300)],
                    p.number))
                .collect::<Vec<_>>()
                .join("\n\n");

            let user = format!(
                r#"Find which page contains the section titled: "{}"

Pages:
{}

Reply in JSON format:
{{"found": true/false, "page": <page_number if found>}}"#,
                title, content
            );

            #[derive(serde::Deserialize)]
            struct SearchResult {
                found: bool,
                page: Option<usize>,
            }

            let result: SearchResult = self.client.complete_json(system, &user).await?;

            if result.found {
                return Ok(result.page);
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_anchors() {
        let assigner = PageAssigner::with_defaults();

        let entries = vec![
            TocEntry::new("Chapter 1", 1).with_toc_page(1),
            TocEntry::new("Chapter 2", 1).with_toc_page(10),
            TocEntry::new("Chapter 3", 1).with_toc_page(20),
            TocEntry::new("Chapter 4", 1).with_toc_page(30),
        ];

        let anchors = assigner.select_anchors(&entries, 2);
        assert_eq!(anchors.len(), 2);
    }

    #[test]
    fn test_calculate_mode() {
        let assigner = PageAssigner::with_defaults();

        let values = vec![2, 2, 2, 3, 3, 4];
        assert_eq!(assigner.calculate_mode(&values), 2);

        let values = vec![1, 1, 2, 2, 2];
        assert_eq!(assigner.calculate_mode(&values), 2);
    }
}
