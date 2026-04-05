// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! TOC processor - integrates all TOC processing components.

use tracing::{debug, info, warn};

use crate::error::Result;
use crate::parser::pdf::PdfPage;

use super::assigner::{PageAssigner, PageAssignerConfig};
use super::detector::{TocDetector, TocDetectorConfig};
use super::parser::{TocParser, TocParserConfig};
use super::repairer::{IndexRepairer, RepairerConfig};
use super::types::{TocEntry, VerificationReport};
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

    /// Accuracy threshold for acceptance.
    pub accuracy_threshold: f32,

    /// Maximum repair attempts.
    pub max_repair_attempts: usize,
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
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::parser::toc::TocProcessor;
/// use vectorless::parser::pdf::PdfParser;
///
/// # #[tokio::main]
/// # async fn main() -> vectorless::domain::Result<()> {
/// // Parse PDF
/// let pdf_parser = PdfParser::new();
/// let result = pdf_parser.parse_file("document.pdf".as_ref())?;
///
/// // Extract TOC
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

    /// Process PDF pages and extract TOC.
    ///
    /// This is the main entry point for TOC extraction.
    pub async fn process(&self, pages: &[PdfPage]) -> Result<Vec<TocEntry>> {
        if pages.is_empty() {
            return Ok(Vec::new());
        }

        info!("Processing {} pages for TOC extraction", pages.len());

        // Step 1: Detect TOC
        let detection = self.detector.detect(pages).await?;
        if !detection.found {
            info!("No TOC found in document");
            return self.process_without_toc(pages).await;
        }

        info!(
            "TOC found on pages {:?}, has_page_numbers: {}",
            detection.pages, detection.has_page_numbers
        );

        // Step 2: Extract TOC text
        let toc_text = self.extract_toc_text(pages, &detection.pages);
        if toc_text.trim().is_empty() {
            warn!("TOC text is empty, falling back to structure extraction");
            return self.process_without_toc(pages).await;
        }

        // Step 3: Parse TOC
        let mut entries = self.parser.parse(&toc_text).await?;
        if entries.is_empty() {
            warn!("No entries parsed from TOC");
            return Ok(Vec::new());
        }

        info!("Parsed {} TOC entries", entries.len());

        // Step 4: Assign physical pages
        self.assigner.assign(&mut entries, pages).await?;

        // Step 5: Verify and repair
        let report = self.verify_and_repair(&mut entries, pages).await?;

        info!(
            "TOC processing complete: {} entries, accuracy {:.1}%",
            entries.len(),
            report.accuracy * 100.0
        );

        Ok(entries)
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

    /// Process document without TOC (structure extraction).
    async fn process_without_toc(&self, pages: &[PdfPage]) -> Result<Vec<TocEntry>> {
        warn!("Processing without TOC - this is a placeholder implementation");

        // TODO: Implement structure extraction for documents without TOC
        // For now, return a simple structure based on page count

        let mut entries = Vec::new();

        // Group pages into chunks
        let chunk_size = 10;
        for chunk in pages.chunks(chunk_size) {
            let start_page = chunk.first().map(|p| p.number).unwrap_or(1);
            let end_page = chunk.last().map(|p| p.number).unwrap_or(1);

            let title = if chunk.len() == 1 {
                format!("Page {}", start_page)
            } else {
                format!("Pages {}-{}", start_page, end_page)
            };

            entries.push(
                TocEntry::new(title, 1)
                    .with_physical_page(start_page)
                    .with_confidence(0.5),
            );
        }

        Ok(entries)
    }

    /// Verify entries and repair if needed.
    async fn verify_and_repair(
        &self,
        entries: &mut [TocEntry],
        pages: &[PdfPage],
    ) -> Result<VerificationReport> {
        let mut attempts = 0;

        while attempts < self.config.max_repair_attempts {
            // Verify
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

            // Repair
            let repaired = self.repairer.repair(entries, &report.errors, pages).await?;

            if repaired == 0 {
                debug!("No repairs possible");
                return Ok(report);
            }

            attempts += 1;
            debug!("Repair attempt {} complete", attempts);
        }

        // Final verification
        self.verifier.verify(entries, pages).await
    }
}

impl Default for TocProcessor {
    fn default() -> Self {
        Self::new()
    }
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
