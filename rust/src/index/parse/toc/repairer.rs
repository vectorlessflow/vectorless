// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index repairer - fixes incorrect TOC entry page assignments.

use futures::future::join_all;
use tracing::{debug, info};

use crate::config::LlmConfig;
use crate::error::Result;
use crate::index::parse::pdf::PdfPage;

use super::types::{TocEntry, VerificationError, VerificationReport};
use super::verifier::IndexVerifier;
use crate::llm::LlmClient;

/// Repairer configuration.
#[derive(Debug, Clone)]
pub struct RepairerConfig {
    /// Maximum repair attempts.
    pub max_attempts: usize,

    /// LLM configuration.
    pub llm_config: LlmConfig,

    /// Page search range around expected page.
    pub search_range: usize,
}

impl Default for RepairerConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            llm_config: LlmConfig::default(),
            search_range: 5,
        }
    }
}

/// Index repairer - fixes incorrect page assignments.
pub struct IndexRepairer {
    config: RepairerConfig,
    client: LlmClient,
}

impl IndexRepairer {
    /// Create a new repairer.
    pub fn new(config: RepairerConfig) -> Self {
        let client = LlmClient::new(config.llm_config.clone().into());
        Self { config, client }
    }

    /// Create a repairer with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(RepairerConfig::default())
    }

    /// Repair incorrect entries concurrently.
    pub async fn repair(
        &self,
        entries: &mut [TocEntry],
        errors: &[VerificationError],
        pages: &[PdfPage],
    ) -> Result<usize> {
        if errors.is_empty() {
            return Ok(0);
        }

        info!("Repairing {} incorrect entries", errors.len());

        // Collect repair tasks (don't borrow entries mutably yet)
        let client = self.client.clone();
        let pages_owned = pages.to_vec();
        let search_range = self.config.search_range;

        let tasks: Vec<_> = errors
            .iter()
            .filter(|error| error.index < entries.len())
            .map(|error| {
                let title = entries[error.index].title.clone();
                let expected_page = error.expected_page;
                let client = client.clone();
                let pages = pages_owned.clone();

                async move {
                    let start = expected_page.saturating_sub(search_range).max(1);
                    let end = (expected_page + search_range).min(pages.len());

                    let result = Self::find_correct_page_static(
                        &client,
                        &title,
                        &pages,
                        start..=end,
                    )
                    .await;

                    (title, expected_page, result)
                }
            })
            .collect();

        let results = join_all(tasks).await;

        // Apply repairs
        let mut repaired_count = 0;
        for (title, expected_page, result) in results {
            match result {
                Ok(Some(correct_page)) => {
                    // Find the corresponding error entry and fix it
                    if let Some(error) = errors.iter().find(|e| e.title == title) {
                        if error.index < entries.len() {
                            debug!(
                                "Repaired '{}' : page {} → {}",
                                title, expected_page, correct_page
                            );
                            entries[error.index].physical_page = Some(correct_page);
                            entries[error.index].confidence = 0.9;
                            repaired_count += 1;
                        }
                    }
                }
                Ok(None) => {
                    debug!(
                        "Could not repair '{}' (searched around page {})",
                        title, expected_page
                    );
                }
                Err(e) => {
                    debug!("Repair failed for '{}': {}", title, e);
                }
            }
        }

        info!("Repaired {}/{} entries", repaired_count, errors.len());
        Ok(repaired_count)
    }

    /// Find the correct page for a title within a range (static, for concurrent use).
    async fn find_correct_page_static(
        client: &LlmClient,
        title: &str,
        pages: &[PdfPage],
        range: std::ops::RangeInclusive<usize>,
    ) -> Result<Option<usize>> {
        let system = "You are a document analysis assistant. Find which page contains a specific section title.";

        // Build content for pages in range
        let mut content_parts = Vec::new();
        for page_num in range {
            if let Some(page) = pages.get(page_num - 1) {
                let text = if page.text.len() > 500 {
                    &page.text[..500]
                } else {
                    &page.text
                };
                content_parts.push(format!(
                    "<page_{}>\n{}\n</page_{}>",
                    page_num, text, page_num
                ));
            }
        }

        if content_parts.is_empty() {
            return Ok(None);
        }

        let content = content_parts.join("\n\n");
        let user = format!(
            r#"Find which page contains the section titled: "{}"

Pages:
{}

Reply in JSON format:
{{"found": true/false, "page": <page_number if found>}}"#,
            title, content
        );

        #[derive(serde::Deserialize)]
        struct FindResult {
            found: bool,
            page: Option<usize>,
        }

        let result: FindResult = client.complete_json(system, &user).await?;

        if result.found {
            Ok(result.page)
        } else {
            Ok(None)
        }
    }

    /// Repair with verification loop.
    pub async fn repair_with_verification(
        &self,
        entries: &mut [TocEntry],
        pages: &[PdfPage],
        verifier: &IndexVerifier,
    ) -> Result<VerificationReport> {
        let mut attempts = 0;
        let threshold = 0.6; // Hardcoded for now, should be from verifier config

        while attempts < self.config.max_attempts {
            // Verify current state
            let report = verifier.verify(entries, pages).await?;

            if report.accuracy >= threshold {
                info!("Repair complete: accuracy {:.1}%", report.accuracy * 100.0);
                return Ok(report);
            }

            if report.errors.is_empty() {
                return Ok(report);
            }

            // Repair errors
            let repaired = self.repair(entries, &report.errors, pages).await?;

            if repaired == 0 {
                // No repairs made, stop trying
                debug!("No repairs possible, stopping");
                return Ok(report);
            }

            attempts += 1;
            info!("Repair attempt {} complete, re-verifying", attempts);
        }

        // Final verification
        verifier.verify(entries, pages).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repairer_creation() {
        let repairer = IndexRepairer::with_defaults();
        assert_eq!(repairer.config.max_attempts, 3);
    }
}
