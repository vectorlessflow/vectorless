// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Parse stage - Parse documents into raw nodes.

use super::async_trait;
use std::time::Instant;
use tracing::info;

use crate::core::Result;
use crate::parser::DocumentFormat;
use crate::parser::ParserRegistry;

use super::{IndexStage, StageResult};
use crate::core::index::pipeline::{IndexContext, IndexInput};
use crate::core::index::IndexMode;

/// Parse stage - extracts raw nodes from documents.
pub struct ParseStage {
    parser_registry: ParserRegistry,
}

impl ParseStage {
    /// Create a new parse stage.
    pub fn new() -> Self {
        Self {
            parser_registry: ParserRegistry::with_defaults(),
        }
    }

    /// Detect document format from path and options.
    fn detect_format(&self, ctx: &IndexContext) -> Result<DocumentFormat> {
        match ctx.options.mode {
            IndexMode::Auto => {
                match &ctx.input {
                    IndexInput::File(path) => {
                        let ext = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("");
                        DocumentFormat::from_extension(ext)
                            .ok_or_else(|| crate::core::Error::Parse(format!("Unknown format: {}", ext)))
                    }
                    IndexInput::Content { format, .. } => Ok(*format),
                }
            }
            IndexMode::Markdown => Ok(DocumentFormat::Markdown),
            IndexMode::Pdf => Ok(DocumentFormat::Pdf),
            IndexMode::Docx => Ok(DocumentFormat::Docx),
            IndexMode::Html => Ok(DocumentFormat::Html),
        }
    }
}

impl Default for ParseStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexStage for ParseStage {
    fn name(&self) -> &str {
        "parse"
    }

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        let start = Instant::now();

        // Detect format
        let format = self.detect_format(ctx)?;
        ctx.format = format;

        info!("Parsing document with format: {:?}", format);

        // Parse based on input type
        let result = match &ctx.input {
            IndexInput::File(path) => {
                // Resolve path
                let path = path.canonicalize().unwrap_or_else(|_| path.clone());
                ctx.source_path = Some(path.clone());

                // Extract name from file
                ctx.name = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("document")
                    .to_string();

                // Parse using registry
                self.parser_registry.parse_file(&path).await?
            }
            IndexInput::Content { content, name, format } => {
                // Set name
                ctx.name = name.clone();

                // Parse content directly
                self.parser_registry.parse(content, *format).await?
            }
        };

        // Store results
        ctx.raw_nodes = result.nodes;
        ctx.metrics.set_nodes_processed(ctx.raw_nodes.len());

        // Store metadata
        if let Some(page_count) = result.meta.page_count {
            ctx.page_count = Some(page_count);
        }
        ctx.line_count = Some(result.meta.line_count);

        if let Some(desc) = result.meta.description {
            ctx.description = Some(desc);
        }

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_parse(duration);

        info!(
            "Parsed {} nodes from {} ({}ms)",
            ctx.raw_nodes.len(),
            ctx.name,
            duration
        );

        let mut stage_result = StageResult::success("parse");
        stage_result.duration_ms = duration;
        stage_result.metadata.insert(
            "node_count".to_string(),
            serde_json::json!(ctx.raw_nodes.len()),
        );
        stage_result.metadata.insert(
            "format".to_string(),
            serde_json::json!(format.extension()),
        );

        Ok(stage_result)
    }
}
