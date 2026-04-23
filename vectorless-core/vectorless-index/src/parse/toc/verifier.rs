// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index verifier - verifies TOC entry page assignments.

use futures::stream::{self, StreamExt};
use rand::seq::SliceRandom;
use tracing::{debug, info};

use vectorless_error::Result;
use crate::index::parse::pdf::PdfPage;
use vectorless_llm::config::LlmConfig;

use super::types::{ErrorType, TocEntry, VerificationError, VerificationReport};
use vectorless_llm::LlmClient;

/// Verifier configuration.
#[derive(Debug, Clone)]
pub struct VerifierConfig {
    /// Sample size for verification (None = all entries).
    pub sample_size: Option<usize>,

    /// LLM configuration.
    pub llm_config: LlmConfig,

    /// Accuracy threshold for acceptance.
    pub accuracy_threshold: f32,
}

impl Default for VerifierConfig {
    fn default() -> Self {
        Self {
            sample_size: Some(10),
            llm_config: LlmConfig::default(),
            accuracy_threshold: 0.6,
        }
    }
}

/// Index verifier - verifies that TOC entries point to correct pages.
pub struct IndexVerifier {
    config: VerifierConfig,
    client: LlmClient,
}

impl IndexVerifier {
    /// Create a new verifier.
    pub fn new(config: VerifierConfig) -> Self {
        let client = LlmClient::new(config.llm_config.clone().into());
        Self { config, client }
    }

    /// Create a verifier with an externally provided LLM client.
    pub fn with_client(client: LlmClient) -> Self {
        Self {
            config: VerifierConfig::default(),
            client,
        }
    }

    /// Create a verifier with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(VerifierConfig::default())
    }

    /// Verify TOC entries against PDF pages.
    ///
    /// Sample entries are verified via LLM calls with bounded concurrency.
    pub async fn verify(
        &self,
        entries: &[TocEntry],
        pages: &[PdfPage],
    ) -> Result<VerificationReport> {
        if entries.is_empty() {
            return Ok(VerificationReport::all_correct(0));
        }

        let sample = self.select_sample(entries);

        // Launch verification checks with bounded concurrency
        let client = self.client.clone();
        let futures: Vec<_> = sample
            .iter()
            .map(|(index, entry)| {
                let index = *index;
                let title = entry.title.clone();
                let physical_page = entry.physical_page;
                let client = client.clone();
                let pages = pages.to_vec();

                async move {
                    match physical_page {
                        Some(page) => {
                            let result =
                                Self::verify_entry_with_client(&client, &title, page, &pages).await;
                            (index, title, page, result)
                        }
                        None => (index, title, 0, Ok(Err(ErrorType::PageOutOfRange))),
                    }
                }
            })
            .collect();

        let results: Vec<_> = stream::iter(futures).buffer_unordered(5).collect().await;

        // Aggregate results
        let total = results.len();
        let mut errors = Vec::new();
        let mut correct = 0;

        for (index, title, page, result) in results {
            match result {
                Ok(Ok(())) => correct += 1,
                Ok(Err(error_type)) => {
                    errors.push(VerificationError::new(index, title, page, error_type));
                }
                Err(e) => {
                    debug!("Verification LLM call failed: {}", e);
                    errors.push(VerificationError::new(
                        index,
                        title,
                        page,
                        ErrorType::TitleNotFound,
                    ));
                }
            }
        }

        let report = VerificationReport::new(total, correct, errors);
        info!(
            "Verification complete: {}/{} correct ({:.1}% accuracy)",
            report.correct,
            report.total,
            report.accuracy * 100.0
        );

        Ok(report)
    }

    /// Select a sample of entries to verify.
    fn select_sample<'a>(&self, entries: &'a [TocEntry]) -> Vec<(usize, &'a TocEntry)> {
        let with_pages: Vec<_> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.physical_page.is_some())
            .collect();

        match self.config.sample_size {
            Some(size) if size < with_pages.len() => {
                // Random sample
                let mut rng = rand::thread_rng();
                let mut sample: Vec<_> = with_pages;
                sample.shuffle(&mut rng);
                sample.into_iter().take(size).collect()
            }
            _ => with_pages,
        }
    }

    /// Verify a single entry using a cloned client (for concurrent use).
    async fn verify_entry_with_client(
        client: &LlmClient,
        title: &str,
        physical_page: usize,
        pages: &[PdfPage],
    ) -> Result<std::result::Result<(), ErrorType>> {
        if physical_page == 0 || physical_page > pages.len() {
            return Ok(Err(ErrorType::PageOutOfRange));
        }

        let page = &pages[physical_page - 1];

        let found = Self::check_title_on_page_with_client(client, title, &page.text).await?;

        if !found {
            debug!("Title '{}' not found on page {}", title, physical_page);
            return Ok(Err(ErrorType::TitleNotFound));
        }

        Ok(Ok(()))
    }

    /// Check if a title appears on a page using LLM.
    async fn check_title_on_page_with_client(
        client: &LlmClient,
        title: &str,
        page_text: &str,
    ) -> Result<bool> {
        let system = "You are a document analysis assistant. Determine if a section title appears in the given text.";

        let text = if page_text.len() > 1000 {
            &page_text[..1000]
        } else {
            page_text
        };

        let user = format!(
            r#"Does the section title "{}" appear in this page text?

Page text:
{}

Reply in JSON format:
{{"found": true/false}}"#,
            title, text
        );

        #[derive(serde::Deserialize)]
        struct CheckResult {
            found: bool,
        }

        let result: CheckResult = client.complete_json(system, &user).await?;
        Ok(result.found)
    }

    /// Check if a title appears at the start of a page.
    pub async fn check_title_at_start(&self, title: &str, page_text: &str) -> Result<bool> {
        let system = "You are a document analysis assistant. Determine if a section title appears at the START of the given page text.";

        // Only check first 500 characters
        let text = if page_text.len() > 500 {
            &page_text[..500]
        } else {
            page_text
        };

        let user = format!(
            r#"Does the section title "{}" appear at the BEGINNING of this page text?
Note: It should be near the start, not in the middle or end.

Page text:
{}

Reply in JSON format:
{{"at_start": true/false}}"#,
            title, text
        );

        #[derive(serde::Deserialize)]
        struct StartCheck {
            at_start: bool,
        }

        let result: StartCheck = self.client.complete_json(system, &user).await?;
        Ok(result.at_start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_sample() {
        let verifier = IndexVerifier::with_defaults();

        let entries: Vec<TocEntry> = (1..=20)
            .map(|i| TocEntry::new(format!("Entry {}", i), 1).with_physical_page(i))
            .collect();

        let sample = verifier.select_sample(&entries);
        assert_eq!(sample.len(), 10); // default sample_size
    }

    #[test]
    fn test_select_sample_all() {
        let config = VerifierConfig {
            sample_size: None,
            ..Default::default()
        };
        let verifier = IndexVerifier::new(config);

        let entries: Vec<TocEntry> = (1..=5)
            .map(|i| TocEntry::new(format!("Entry {}", i), 1).with_physical_page(i))
            .collect();

        let sample = verifier.select_sample(&entries);
        assert_eq!(sample.len(), 5);
    }
}
