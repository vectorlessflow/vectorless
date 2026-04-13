// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! TOC processor - integrates all TOC processing components.
//!
//! The processor orchestrates a multi-mode extraction pipeline with automatic
//! degradation: if one mode fails verification, it falls back to a lower-quality
//! but more reliable mode.

use tracing::{debug, info, warn};

use crate::error::Result;
use crate::index::parse::pdf::PdfPage;

use super::assigner::{PageAssigner, PageAssignerConfig};
use super::detector::{TocDetector, TocDetectorConfig};
use super::parser::{TocParser, TocParserConfig};
use super::repairer::{IndexRepairer, RepairerConfig};
use super::structure_extractor::{StructureExtractor, StructureExtractorConfig};
use super::types::{ProcessingMode, TocEntry, VerificationReport};
use super::verifier::{IndexVerifier, VerifierConfig};

/// TOC processor configuration.
#[derive(Debug, Clone)]
pub struct TocProcessorConfig {
    /// TOC detector configuration.
    pub detector: TocDetectorConfig,

    /// TOC parser configuration.
    pub parser: TocParserConfig,

    /// Page assigner configuration.
    pub assigner: PageAssignerConfig,

    /// Verifier configuration.
    pub verifier: VerifierConfig,

    /// Repairer configuration.
    pub repairer: RepairerConfig,

    /// Accuracy threshold for acceptance (0.0 - 1.0).
    pub accuracy_threshold: f32,

    /// Maximum repair attempts per verification cycle.
    pub max_repair_attempts: usize,

    /// Maximum page span for a single entry before recursive refinement.
    pub max_pages_per_entry: usize,

    /// Maximum estimated tokens for a single entry before recursive refinement.
    pub max_tokens_per_entry: usize,
}

impl Default for TocProcessorConfig {
    fn default() -> Self {
        Self {
            detector: TocDetectorConfig::default(),
            parser: TocParserConfig::default(),
            assigner: PageAssignerConfig::default(),
            verifier: VerifierConfig::default(),
            repairer: RepairerConfig::default(),
            accuracy_threshold: 0.6,
            max_repair_attempts: 3,
            max_pages_per_entry: 30,
            max_tokens_per_entry: 20000,
        }
    }
}

/// TOC processor - orchestrates the complete TOC extraction pipeline.
///
/// # Processing Pipeline
///
/// 1. **Detect** - Find TOC in document (regex + LLM fallback)
/// 2. **Extract** - Get TOC text from detected pages
/// 3. **Parse** - Convert TOC text to structured entries (LLM)
/// 4. **Assign** - Map TOC pages to physical pages
/// 5. **Verify** - Sample verification of page assignments
/// 6. **Repair** - Fix incorrect assignments (if needed)
/// 7. **Refine** - Sub-divide oversized entries (if needed)
///
/// # Degradation Strategy
///
/// The pipeline tries three modes in order of quality:
///
/// 1. `TocWithPageNumbers` - TOC found with page numbers (offset calculation)
/// 2. `TocWithoutPageNumbers` - TOC found without page numbers (LLM positioning)
/// 3. `NoToc` - No TOC available (LLM structure extraction from content)
///
/// If a mode fails verification (accuracy < threshold), it automatically
/// degrades to the next mode.
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::parser::toc::TocProcessor;
/// use vectorless::parser::pdf::PdfParser;
///
/// # #[tokio::main]
/// # async fn main() -> vectorless::Result<()> {
/// let pdf_parser = PdfParser::new();
/// let result = pdf_parser.parse_file("document.pdf".as_ref()).await?;
///
/// let processor = TocProcessor::new();
/// let entries = processor.process(&result.pages).await?;
///
/// for entry in &entries {
///     println!("{} - Page {:?}", entry.title, entry.physical_page);
/// }
/// # Ok(())
/// # }
/// ```
pub struct TocProcessor {
    config: TocProcessorConfig,
    detector: TocDetector,
    parser: TocParser,
    assigner: PageAssigner,
    verifier: IndexVerifier,
    repairer: IndexRepairer,
}

impl TocProcessor {
    /// Create a new TOC processor with default configuration.
    pub fn new() -> Self {
        Self::with_config(TocProcessorConfig::default())
    }

    /// Create a TOC processor with custom configuration.
    pub fn with_config(config: TocProcessorConfig) -> Self {
        Self {
            detector: TocDetector::new(config.detector.clone()),
            parser: TocParser::new(config.parser.clone()),
            assigner: PageAssigner::new(config.assigner.clone()),
            verifier: IndexVerifier::new(config.verifier.clone()),
            repairer: IndexRepairer::new(config.repairer.clone()),
            config,
        }
    }

    /// Process PDF pages and extract hierarchical structure.
    ///
    /// This is the main entry point. It detects TOC, selects the best
    /// processing mode, and automatically degrades if needed.
    pub async fn process(&self, pages: &[PdfPage]) -> Result<Vec<TocEntry>> {
        if pages.is_empty() {
            return Ok(Vec::new());
        }

        info!("Processing {} pages for TOC extraction", pages.len());

        // Step 1: Detect TOC
        let detection = self.detector.detect(pages).await?;

        // Step 2: Determine initial mode based on detection result
        let initial_mode = if !detection.found {
            info!("No TOC found in document");
            ProcessingMode::NoToc
        } else if detection.has_page_numbers {
            info!(
                "TOC found on pages {:?}, has page numbers",
                detection.pages
            );
            ProcessingMode::TocWithPageNumbers
        } else {
            info!(
                "TOC found on pages {:?}, no page numbers",
                detection.pages
            );
            ProcessingMode::TocWithoutPageNumbers
        };

        // Step 3: Process with degradation
        let entries = self
            .process_with_degradation(initial_mode, &detection, pages)
            .await?;

        // Step 4: Refine oversized entries
        self.refine_large_entries(entries, pages).await
    }

    /// Process with automatic mode degradation.
    ///
    /// Tries the given mode, verifies the result, and degrades to a
    /// lower-quality mode if accuracy is below threshold.
    async fn process_with_degradation(
        &self,
        initial_mode: ProcessingMode,
        detection: &super::types::TocDetection,
        pages: &[PdfPage],
    ) -> Result<Vec<TocEntry>> {
        let mut mode = initial_mode;

        loop {
            info!("Attempting extraction with mode {:?}", mode);

            let result = match mode {
                ProcessingMode::TocWithPageNumbers => {
                    self.process_toc_with_page_numbers(detection, pages).await
                }
                ProcessingMode::TocWithoutPageNumbers => {
                    self.process_toc_without_page_numbers(detection, pages).await
                }
                ProcessingMode::NoToc => {
                    // NoToc always succeeds (produces some structure)
                    return self.process_without_toc(pages).await;
                }
            };

            match result {
                Ok(entries) if !entries.is_empty() => {
                    // Verify the entries
                    let mut mutable_entries = entries;
                    let report = self
                        .verify_and_repair(&mut mutable_entries, pages)
                        .await?;

                    if report.accuracy >= self.config.accuracy_threshold {
                        info!(
                            "Mode {:?} succeeded: {} entries, accuracy {:.1}%",
                            mode,
                            mutable_entries.len(),
                            report.accuracy * 100.0
                        );
                        return Ok(mutable_entries);
                    }

                    // Accuracy too low, try degrading
                    warn!(
                        "Mode {:?} accuracy {:.1}% below threshold {:.1}%",
                        mode,
                        report.accuracy * 100.0,
                        self.config.accuracy_threshold * 100.0
                    );

                    match mode.degrade() {
                        Some(next) => {
                            info!("Degrading from {:?} to {:?}", mode, next);
                            mode = next;
                            // Continue loop with degraded mode
                        }
                        None => {
                            warn!("No further degradation possible, returning best effort");
                            return Ok(mutable_entries);
                        }
                    }
                }
                Ok(_) => {
                    // Empty entries, degrade
                    warn!("Mode {:?} produced no entries", mode);
                    match mode.degrade() {
                        Some(next) => {
                            mode = next;
                        }
                        None => return Ok(Vec::new()),
                    }
                }
                Err(e) => {
                    warn!("Mode {:?} failed: {}", mode, e);
                    match mode.degrade() {
                        Some(next) => {
                            mode = next;
                        }
                        None => return Err(e),
                    }
                }
            }
        }
    }

    /// Mode 1: TOC with page numbers.
    ///
    /// Parse the TOC, calculate physical-page offset from anchor entries,
    /// and apply the offset to all entries.
    async fn process_toc_with_page_numbers(
        &self,
        detection: &super::types::TocDetection,
        pages: &[PdfPage],
    ) -> Result<Vec<TocEntry>> {
        let toc_text = self.extract_toc_text(pages, &detection.pages);
        if toc_text.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut entries = self.parser.parse(&toc_text).await?;
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // Assign physical pages using offset calculation
        self.assigner.assign(&mut entries, pages).await?;

        Ok(entries)
    }

    /// Mode 2: TOC without page numbers.
    ///
    /// Parse the TOC, then use LLM to locate each entry in the document.
    async fn process_toc_without_page_numbers(
        &self,
        detection: &super::types::TocDetection,
        pages: &[PdfPage],
    ) -> Result<Vec<TocEntry>> {
        let toc_text = self.extract_toc_text(pages, &detection.pages);
        if toc_text.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut entries = self.parser.parse(&toc_text).await?;
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // Clear any TOC page numbers (they're unreliable in this mode)
        for entry in &mut entries {
            entry.toc_page = None;
        }

        // Assign physical pages using LLM positioning
        self.assigner.assign(&mut entries, pages).await?;

        Ok(entries)
    }

    /// Mode 3: No TOC available.
    ///
    /// Extract document structure directly from page content using LLM.
    async fn process_without_toc(&self, pages: &[PdfPage]) -> Result<Vec<TocEntry>> {
        info!("Extracting structure from page content (no TOC available)");

        let extractor = StructureExtractor::new(StructureExtractorConfig::default());
        extractor.extract(pages).await
    }

    /// Extract TOC text from pages.
    fn extract_toc_text(&self, pages: &[PdfPage], toc_pages: &[usize]) -> String {
        toc_pages
            .iter()
            .filter_map(|&page_num| pages.get(page_num - 1))
            .map(|page| page.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Verify entries and repair if needed.
    async fn verify_and_repair(
        &self,
        entries: &mut [TocEntry],
        pages: &[PdfPage],
    ) -> Result<VerificationReport> {
        let mut attempts = 0;

        while attempts < self.config.max_repair_attempts {
            let report = self.verifier.verify(entries, pages).await?;

            if report.accuracy >= self.config.accuracy_threshold {
                debug!(
                    "Verification passed: accuracy {:.1}%",
                    report.accuracy * 100.0
                );
                return Ok(report);
            }

            if report.errors.is_empty() {
                return Ok(report);
            }

            let repaired = self.repairer.repair(entries, &report.errors, pages).await?;

            if repaired == 0 {
                debug!("No repairs possible");
                return Ok(report);
            }

            attempts += 1;
            debug!("Repair attempt {} complete", attempts);
        }

        self.verifier.verify(entries, pages).await
    }

    /// Refine oversized entries by extracting sub-structure.
    ///
    /// Entries that span too many pages or tokens are broken down using
    /// the same structure extraction approach used for no-TOC documents.
    async fn refine_large_entries(
        &self,
        entries: Vec<TocEntry>,
        pages: &[PdfPage],
    ) -> Result<Vec<TocEntry>> {
        if entries.is_empty() {
            return Ok(entries);
        }

        let page_count = pages.len();

        // Pre-compute next-entry page numbers before consuming entries
        let next_pages: Vec<Option<usize>> = entries
            .iter()
            .enumerate()
            .map(|(i, _)| {
                entries.get(i + 1).and_then(|e| e.physical_page)
            })
            .collect();

        let mut refined = Vec::with_capacity(entries.len());

        for (i, entry) in entries.into_iter().enumerate() {
            let span = entry_page_span(&entry, next_pages[i], page_count);
            let tokens = entry_token_count(&entry, pages);

            if span > self.config.max_pages_per_entry
                && tokens > self.config.max_tokens_per_entry
            {
                debug!(
                    "Refining oversized entry '{}' ({} pages, ~{} tokens)",
                    entry.title, span, tokens
                );

                // Extract sub-pages covered by this entry
                let start = entry.physical_page.unwrap_or(1);
                let end = next_pages[i].unwrap_or(page_count);
                let sub_pages: Vec<PdfPage> = pages
                    .iter()
                    .filter(|p| p.number >= start && p.number <= end)
                    .cloned()
                    .collect();

                if sub_pages.is_empty() {
                    refined.push(entry);
                } else {
                    // Run structure extraction on the sub-pages
                    let extractor =
                        StructureExtractor::new(StructureExtractorConfig::default());
                    match extractor.extract(&sub_pages).await {
                        Ok(sub_entries) if !sub_entries.is_empty() => {
                            // If the first sub-entry has the same title as the
                            // parent, skip it — the parent already represents
                            // that content's starting point.
                            let skip = if sub_entries
                                .first()
                                .map(|e| e.title.trim() == entry.title.trim())
                                .unwrap_or(false)
                            {
                                1
                            } else {
                                0
                            };

                            for sub in &sub_entries[skip..] {
                                let level_offset = entry.level;
                                refined.push(
                                    TocEntry::new(&sub.title, sub.level + level_offset)
                                        .with_physical_page(sub.physical_page.unwrap_or(start))
                                        .with_confidence(sub.confidence * 0.9),
                                );
                            }

                            info!(
                                "Refined '{}' into {} sub-entries",
                                entry.title,
                                sub_entries.len() - skip
                            );
                        }
                        Ok(_) => {
                            debug!("Sub-extraction produced no entries, keeping original");
                            refined.push(entry);
                        }
                        Err(e) => {
                            warn!("Sub-extraction failed for '{}': {}", entry.title, e);
                            refined.push(entry);
                        }
                    }
                }
            } else {
                refined.push(entry);
            }
        }

        Ok(refined)
    }
}

impl Default for TocProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate how many pages an entry spans.
///
/// From its physical_page to the next entry's physical_page (or document end).
fn entry_page_span(entry: &TocEntry, next_physical_page: Option<usize>, total_pages: usize) -> usize {
    let start = entry.physical_page.unwrap_or(1);
    let end = next_physical_page.unwrap_or(total_pages);
    end.saturating_sub(start)
}

/// Estimate total tokens for the content covered by an entry.
fn entry_token_count(entry: &TocEntry, pages: &[PdfPage]) -> usize {
    let start = entry.physical_page.unwrap_or(1);
    pages
        .iter()
        .filter(|p| p.number >= start)
        .take(30) // cap at max_pages_per_entry default
        .map(|p| p.token_count)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_creation() {
        let processor = TocProcessor::new();
        assert_eq!(processor.config.accuracy_threshold, 0.6);
    }

    #[tokio::test]
    async fn test_empty_pages() {
        let processor = TocProcessor::new();
        let entries = processor.process(&[]).await.unwrap();
        assert!(entries.is_empty());
    }
}
