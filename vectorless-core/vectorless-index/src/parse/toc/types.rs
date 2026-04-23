// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! TOC (Table of Contents) types.

use serde::{Deserialize, Serialize};

/// A single TOC entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocEntry {
    /// Section title.
    pub title: String,

    /// Hierarchy level (1 = top level, 2 = subsection, etc.).
    pub level: usize,

    /// Page number from TOC (may have offset).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toc_page: Option<usize>,

    /// Actual physical page number (after verification/assignment).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physical_page: Option<usize>,

    /// Confidence score (0.0 - 1.0).
    #[serde(default)]
    pub confidence: f32,

    /// Start line index (for tree building).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_index: Option<usize>,

    /// End line index (for tree building).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_index: Option<usize>,

    /// Content of this section.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub content: String,
}

impl TocEntry {
    /// Create a new TOC entry.
    pub fn new(title: impl Into<String>, level: usize) -> Self {
        Self {
            title: title.into(),
            level,
            toc_page: None,
            physical_page: None,
            confidence: 1.0,
            start_index: None,
            end_index: None,
            content: String::new(),
        }
    }

    /// Set the TOC page number.
    pub fn with_toc_page(mut self, page: usize) -> Self {
        self.toc_page = Some(page);
        self
    }

    /// Set the physical page number.
    pub fn with_physical_page(mut self, page: usize) -> Self {
        self.physical_page = Some(page);
        self
    }

    /// Set the confidence score.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Check if this entry has a valid physical page.
    pub fn has_physical_page(&self) -> bool {
        self.physical_page.is_some()
    }
}

impl Default for TocEntry {
    fn default() -> Self {
        Self::new("", 1)
    }
}

/// Result of TOC detection.
#[derive(Debug, Clone)]
pub struct TocDetection {
    /// Whether a TOC was found.
    pub found: bool,

    /// Page numbers where TOC appears.
    pub pages: Vec<usize>,

    /// Whether the TOC contains page numbers.
    pub has_page_numbers: bool,

    /// Detection confidence (0.0 - 1.0).
    pub confidence: f32,
}

impl TocDetection {
    /// Create a new TOC detection result.
    pub fn new(found: bool) -> Self {
        Self {
            found,
            pages: Vec::new(),
            has_page_numbers: false,
            confidence: 0.0,
        }
    }

    /// Create a result indicating no TOC was found.
    pub fn not_found() -> Self {
        Self::new(false)
    }

    /// Set the TOC pages.
    pub fn with_pages(mut self, pages: Vec<usize>) -> Self {
        self.pages = pages;
        self
    }

    /// Set whether page numbers are present.
    pub fn with_page_numbers(mut self, has: bool) -> Self {
        self.has_page_numbers = has;
        self
    }

    /// Set the confidence score.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }
}

/// Page offset calculation result.
#[derive(Debug, Clone)]
pub struct PageOffset {
    /// Calculated offset: physical_page = toc_page + offset.
    pub offset: i32,

    /// Number of samples used for calculation.
    pub sample_count: usize,

    /// Confidence in the offset calculation.
    pub confidence: f32,
}

impl PageOffset {
    /// Create a new page offset.
    pub fn new(offset: i32, sample_count: usize, confidence: f32) -> Self {
        Self {
            offset,
            sample_count,
            confidence: confidence.clamp(0.0, 1.0),
        }
    }

    /// Apply offset to a TOC page number.
    pub fn apply(&self, toc_page: usize) -> usize {
        (toc_page as i32 + self.offset).max(1) as usize
    }
}

/// Verification error for a single entry.
#[derive(Debug, Clone)]
pub struct VerificationError {
    /// Index of the entry in the TOC list.
    pub index: usize,

    /// Entry title.
    pub title: String,

    /// Expected physical page.
    pub expected_page: usize,

    /// Type of error.
    pub error_type: ErrorType,
}

impl VerificationError {
    /// Create a new verification error.
    pub fn new(
        index: usize,
        title: impl Into<String>,
        expected_page: usize,
        error_type: ErrorType,
    ) -> Self {
        Self {
            index,
            title: title.into(),
            expected_page,
            error_type,
        }
    }
}

/// Type of verification error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorType {
    /// Title not found on the expected page.
    TitleNotFound,
    /// Title found but not at page start.
    NotAtPageStart,
    /// Page number out of document range.
    PageOutOfRange,
}

impl std::fmt::Display for ErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorType::TitleNotFound => write!(f, "Title not found on page"),
            ErrorType::NotAtPageStart => write!(f, "Title not at page start"),
            ErrorType::PageOutOfRange => write!(f, "Page out of range"),
        }
    }
}

/// Result of TOC verification.
#[derive(Debug, Clone)]
pub struct VerificationReport {
    /// Total entries verified.
    pub total: usize,

    /// Number of correct entries.
    pub correct: usize,

    /// Accuracy (0.0 - 1.0).
    pub accuracy: f32,

    /// List of errors found.
    pub errors: Vec<VerificationError>,
}

impl VerificationReport {
    /// Create a new verification report.
    pub fn new(total: usize, correct: usize, errors: Vec<VerificationError>) -> Self {
        let accuracy = if total > 0 {
            correct as f32 / total as f32
        } else {
            1.0
        };
        Self {
            total,
            correct,
            accuracy,
            errors,
        }
    }

    /// Create a report indicating all entries are correct.
    pub fn all_correct(total: usize) -> Self {
        Self::new(total, total, Vec::new())
    }

    /// Check if the accuracy meets a threshold.
    pub fn meets_threshold(&self, threshold: f32) -> bool {
        self.accuracy >= threshold
    }

    /// Check if there are any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Processing mode for the TOC extraction pipeline.
///
/// Modes are ordered by quality: higher modes produce more accurate results
/// when they succeed, but can degrade to lower modes on failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingMode {
    /// TOC found with page numbers. Highest quality path.
    TocWithPageNumbers,
    /// TOC found without page numbers, or page-number accuracy was too low.
    TocWithoutPageNumbers,
    /// No TOC, or all TOC-based modes failed. LLM-driven structure extraction.
    NoToc,
}

impl ProcessingMode {
    /// Degrade to the next lower quality mode.
    ///
    /// Returns `None` if already at the lowest mode (`NoToc`).
    pub fn degrade(self) -> Option<Self> {
        match self {
            Self::TocWithPageNumbers => Some(Self::TocWithoutPageNumbers),
            Self::TocWithoutPageNumbers => Some(Self::NoToc),
            Self::NoToc => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toc_entry() {
        let entry = TocEntry::new("Chapter 1", 1)
            .with_toc_page(10)
            .with_physical_page(12)
            .with_confidence(0.9);

        assert_eq!(entry.title, "Chapter 1");
        assert_eq!(entry.level, 1);
        assert_eq!(entry.toc_page, Some(10));
        assert_eq!(entry.physical_page, Some(12));
        assert!((entry.confidence - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_page_offset() {
        let offset = PageOffset::new(2, 5, 0.9);
        assert_eq!(offset.apply(10), 12);
        assert_eq!(offset.apply(1), 3);
    }

    #[test]
    fn test_verification_report() {
        let report = VerificationReport::all_correct(10);
        assert_eq!(report.total, 10);
        assert_eq!(report.correct, 10);
        assert_eq!(report.accuracy, 1.0);
        assert!(!report.has_errors());
    }

    #[test]
    fn test_error_type_display() {
        assert_eq!(
            format!("{}", ErrorType::TitleNotFound),
            "Title not found on page"
        );
    }

    #[test]
    fn test_processing_mode_degrade() {
        assert_eq!(
            ProcessingMode::TocWithPageNumbers.degrade(),
            Some(ProcessingMode::TocWithoutPageNumbers)
        );
        assert_eq!(
            ProcessingMode::TocWithoutPageNumbers.degrade(),
            Some(ProcessingMode::NoToc)
        );
        assert_eq!(ProcessingMode::NoToc.degrade(), None);
    }
}
