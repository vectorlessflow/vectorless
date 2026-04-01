// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! TOC parser - converts TOC text to structured entries.

use tracing::debug;

use crate::config::LlmConfig;
use crate::core::Result;

use super::llm_client::LlmClient;
use super::types::TocEntry;

/// TOC parser configuration.
#[derive(Debug, Clone)]
pub struct TocParserConfig {
    /// LLM configuration.
    pub llm_config: LlmConfig,

    /// Maximum retries for incomplete parsing.
    pub max_retries: usize,

    /// Verify completeness after parsing.
    pub verify_completeness: bool,
}

impl Default for TocParserConfig {
    fn default() -> Self {
        Self {
            llm_config: LlmConfig::default(),
            max_retries: 3,
            verify_completeness: true,
        }
    }
}

/// TOC parser - converts raw TOC text to structured entries.
pub struct TocParser {
    config: TocParserConfig,
    client: LlmClient,
}

impl TocParser {
    /// Create a new TOC parser.
    pub fn new(config: TocParserConfig) -> Self {
        let client = LlmClient::new(config.llm_config.clone().into());
        Self { config, client }
    }

    /// Create a parser with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(TocParserConfig::default())
    }

    /// Parse TOC text into structured entries.
    pub async fn parse(&self, toc_text: &str) -> Result<Vec<TocEntry>> {
        if toc_text.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Step 1: Initial parse
        let entries = self.parse_with_llm(toc_text).await?;
        debug!("Initial parse: {} entries", entries.len());

        if entries.is_empty() {
            return Ok(entries);
        }

        // Step 2: Verify completeness (if enabled)
        if self.config.verify_completeness {
            self.verify_and_complete(toc_text, entries).await
        } else {
            Ok(entries)
        }
    }

    /// Parse TOC text using LLM.
    async fn parse_with_llm(&self, toc_text: &str) -> Result<Vec<TocEntry>> {
        let system = r#"You are a document structure extraction expert.
Your task is to parse a Table of Contents (TOC) into a structured format.

Rules:
1. Extract all sections and subsections
2. Determine the hierarchy level (1 = top level, 2 = subsection, etc.)
3. Extract page numbers if present
4. Preserve original titles exactly (only fix spacing issues)
5. If the TOC seems incomplete, extract what you can see"#;

        let user = format!(
            r#"Parse this Table of Contents:

{}

Return a JSON array:
[
  {{
    "title": "Section Title",
    "level": 1,
    "page": 10
  }},
  ...
]

Notes:
- "level" should reflect the hierarchy (1, 2, 3...)
- "page" is optional if not present in TOC
- Only output the JSON array, no other text"#,
            toc_text
        );

        #[derive(serde::Deserialize)]
        struct ParsedEntry {
            title: String,
            level: usize,
            #[serde(default)]
            page: Option<usize>,
        }

        let entries: Vec<ParsedEntry> = self.client.complete_json(system, &user).await?;

        Ok(entries
            .into_iter()
            .map(|e| {
                let mut entry = TocEntry::new(e.title, e.level);
                if let Some(page) = e.page {
                    entry = entry.with_toc_page(page);
                }
                entry
            })
            .collect())
    }

    /// Verify completeness and continue if needed.
    async fn verify_and_complete(&self, toc_text: &str, mut entries: Vec<TocEntry>) -> Result<Vec<TocEntry>> {
        let mut attempts = 0;

        while attempts < self.config.max_retries {
            // Check if parsing is complete
            let is_complete = self.check_completeness(toc_text, &entries).await?;

            if is_complete {
                debug!("TOC parsing complete after {} attempts", attempts + 1);
                return Ok(entries);
            }

            debug!("TOC incomplete, attempting continuation (attempt {})", attempts + 1);

            // Continue parsing
            let additional = self.continue_parsing(toc_text, &entries).await?;
            if additional.is_empty() {
                // No more entries found, stop
                break;
            }

            entries.extend(additional);
            attempts += 1;
        }

        Ok(entries)
    }

    /// Check if parsing is complete.
    async fn check_completeness(&self, toc_text: &str, entries: &[TocEntry]) -> Result<bool> {
        let system = "You are a document analysis assistant. Determine if the parsed entries completely represent the original TOC.";

        let entries_json = serde_json::to_string_pretty(
            &entries.iter().map(|e| &e.title).collect::<Vec<_>>()
        ).unwrap_or_default();

        let user = format!(
            r#"Original TOC:
{}

Parsed entries:
{}

Is the parsing complete? Reply with JSON:
{{"complete": true/false}}"#,
            toc_text, entries_json
        );

        #[derive(serde::Deserialize)]
        struct CompletenessCheck {
            complete: bool,
        }

        let result: CompletenessCheck = self.client.complete_json(system, &user).await?;
        Ok(result.complete)
    }

    /// Continue parsing from where we left off.
    async fn continue_parsing(&self, toc_text: &str, existing: &[TocEntry]) -> Result<Vec<TocEntry>> {
        let system = "You are a document structure extraction expert. Continue parsing the TOC from where it was left off.";

        let last_titles: Vec<_> = existing.iter().rev().take(5).map(|e| &e.title).collect();

        let user = format!(
            r#"Original TOC:
{}

Already parsed (last 5):
{:?}

Extract the REMAINING entries that were missed. Return a JSON array:
[
  {{"title": "...", "level": N, "page": M}},
  ...
]

If nothing was missed, return an empty array: []"#,
            toc_text, last_titles
        );

        #[derive(serde::Deserialize)]
        struct ParsedEntry {
            title: String,
            level: usize,
            #[serde(default)]
            page: Option<usize>,
        }

        let entries: Vec<ParsedEntry> = self.client.complete_json(system, &user).await?;

        Ok(entries
            .into_iter()
            .map(|e| {
                let mut entry = TocEntry::new(e.title, e.level);
                if let Some(page) = e.page {
                    entry = entry.with_toc_page(page);
                }
                entry
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_simple_toc() {
        let parser = TocParser::with_defaults();

        // This test requires an API key
        if std::env::var("OPENAI_API_KEY").is_err() {
            return;
        }

        let toc_text = r#"
Chapter 1. Introduction  1
  1.1 Background  2
  1.2 Objectives  5
Chapter 2. Methods  10
"#;

        let entries = parser.parse(toc_text).await.unwrap();
        assert!(!entries.is_empty());
    }
}
