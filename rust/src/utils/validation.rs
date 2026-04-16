// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Source validation utilities for indexing.

use std::path::Path;

use crate::error::{Error, Result};
use crate::index::parse::DocumentFormat;

/// Maximum file size before emitting a warning (100 MB).
const LARGE_FILE_THRESHOLD: usize = 100 * 1024 * 1024;

/// Result of validating a source before indexing.
#[derive(Debug, Clone)]
pub struct SourceValidation {
    /// Whether the source is valid for indexing.
    pub valid: bool,

    /// Validation errors (prevents indexing).
    pub errors: Vec<String>,

    /// Validation warnings (non-blocking).
    pub warnings: Vec<String>,
}

impl SourceValidation {
    fn valid() -> Self {
        Self {
            valid: true,
            errors: vec![],
            warnings: vec![],
        }
    }

    fn invalid(errors: Vec<String>) -> Self {
        Self {
            valid: false,
            errors,
            warnings: vec![],
        }
    }

    fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }
}

/// Validate a file path for indexing.
///
/// Checks: exists, readable, supported format, size.
pub fn validate_file(path: &Path) -> Result<SourceValidation> {
    if !path.exists() {
        return Ok(SourceValidation::invalid(vec![format!(
            "File not found: {}",
            path.display()
        )]));
    }

    let metadata = std::fs::metadata(path)
        .map_err(|e| Error::Parse(format!("Cannot read file metadata: {}", e)))?;

    let size = metadata.len() as usize;
    let mut warnings = Vec::new();

    if size > LARGE_FILE_THRESHOLD {
        warnings.push(format!(
            "Large file ({}MB) may take longer to index",
            size / (1024 * 1024)
        ));
    }

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if DocumentFormat::from_extension(ext).is_none() {
        return Ok(
            SourceValidation::invalid(vec![format!("Unsupported format: .{}", ext)])
                .with_warnings(warnings),
        );
    }

    Ok(SourceValidation::valid().with_warnings(warnings))
}

/// Validate content string for indexing.
///
/// Checks: non-empty.
pub fn validate_content(content: &str, _format: DocumentFormat) -> SourceValidation {
    let mut errors = Vec::new();

    if content.trim().is_empty() {
        errors.push("Content is empty".to_string());
    }

    if errors.is_empty() {
        SourceValidation::valid()
    } else {
        SourceValidation::invalid(errors)
    }
}

/// Validate binary data for indexing.
///
/// Checks: non-empty, PDF magic number.
pub fn validate_bytes(data: &[u8], format: DocumentFormat) -> SourceValidation {
    let mut errors = Vec::new();

    if data.is_empty() {
        errors.push("Byte data is empty".to_string());
    }

    // PDF magic number check
    if format == DocumentFormat::Pdf && !data.is_empty() {
        if !data.starts_with(b"%PDF") {
            errors.push("Data does not appear to be a valid PDF (missing %PDF header)".to_string());
        }
    }

    if errors.is_empty() {
        SourceValidation::valid()
    } else {
        SourceValidation::invalid(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_file_missing() {
        let result = validate_file(Path::new("./nonexistent.md")).unwrap();
        assert!(!result.valid);
        assert!(result.errors[0].contains("not found"));
    }

    #[test]
    fn test_validate_file_unsupported_format() {
        let tmp = std::env::temp_dir().join("vectorless_test_validate.dat");
        std::fs::write(&tmp, b"data").unwrap();
        let result = validate_file(&tmp).unwrap();
        assert!(!result.valid);
        assert!(result.errors[0].contains("Unsupported"));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_validate_file_valid() {
        let tmp = std::env::temp_dir().join("vectorless_test_validate.md");
        std::fs::write(&tmp, b"# Hello").unwrap();
        let result = validate_file(&tmp).unwrap();
        assert!(result.valid);
        assert!(result.errors.is_empty());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_validate_content_empty() {
        let result = validate_content("  \n  ", DocumentFormat::Markdown);
        assert!(!result.valid);
        assert!(result.errors[0].contains("empty"));
    }

    #[test]
    fn test_validate_content_valid() {
        let result = validate_content("# Hello", DocumentFormat::Markdown);
        assert!(result.valid);
    }

    #[test]
    fn test_validate_bytes_empty() {
        let result = validate_bytes(&[], DocumentFormat::Pdf);
        assert!(!result.valid);
        assert!(result.errors[0].contains("empty"));
    }

    #[test]
    fn test_validate_bytes_invalid_pdf() {
        let result = validate_bytes(b"not a pdf", DocumentFormat::Pdf);
        assert!(!result.valid);
        assert!(result.errors[0].contains("PDF"));
    }

    #[test]
    fn test_validate_bytes_valid_pdf() {
        let result = validate_bytes(b"%PDF-1.4 some content", DocumentFormat::Pdf);
        assert!(result.valid);
    }

    #[test]
    fn test_validate_bytes_valid_markdown() {
        let result = validate_bytes(b"# Hello", DocumentFormat::Markdown);
        assert!(result.valid);
    }
}
