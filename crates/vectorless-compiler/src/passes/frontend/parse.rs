// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Parse stage - Parse documents into raw nodes.

use crate::passes::async_trait;
use std::time::Instant;
use tracing::{debug, info};

use vectorless_document::DocumentFormat;
use vectorless_error::Result;

use crate::SourceFormat;
use crate::parse::ParserRegistry;
use crate::passes::{CompilePass, PassResult};
use crate::pipeline::{CompileContext, CompilerInput};

/// Parse stage - extracts raw nodes from documents.
pub struct ParsePass {
    /// Optional LLM client for PDF structure extraction.
    llm_client: Option<vectorless_llm::LlmClient>,
    /// Parser registry for format dispatch.
    registry: ParserRegistry,
}

impl ParsePass {
    /// Create a new parse stage with default parsers.
    pub fn new() -> Self {
        Self {
            llm_client: None,
            registry: ParserRegistry::default_parsers(None),
        }
    }

    /// Create a parse stage with an LLM client.
    pub fn with_llm_client(client: vectorless_llm::LlmClient) -> Self {
        Self {
            llm_client: Some(client.clone()),
            registry: ParserRegistry::default_parsers(Some(client)),
        }
    }

    /// Create a parse stage with a custom parser registry.
    pub fn with_registry(registry: ParserRegistry) -> Self {
        Self {
            llm_client: None,
            registry,
        }
    }

    /// Detect document format from path and options.
    fn detect_format(&self, ctx: &CompileContext) -> Result<DocumentFormat> {
        match &ctx.options.mode {
            SourceFormat::Auto => match &ctx.input {
                CompilerInput::File(path) => {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    DocumentFormat::from_extension(ext).ok_or_else(|| {
                        vectorless_error::Error::Parse(format!("Unknown format: {}", ext))
                    })
                }
                CompilerInput::Content { format, .. } => Ok(format.clone()),
                CompilerInput::Bytes { format, .. } => Ok(format.clone()),
                CompilerInput::PreParsed { .. } => Ok(DocumentFormat::Markdown),
            },
            SourceFormat::Markdown => Ok(DocumentFormat::Markdown),
            SourceFormat::Pdf => Ok(DocumentFormat::Pdf),
            SourceFormat::Custom(name) => Ok(DocumentFormat::Custom(name.clone())),
        }
    }

    /// Resolve format name for registry lookup.
    fn format_name(format: &DocumentFormat) -> &str {
        match format {
            DocumentFormat::Markdown => "markdown",
            DocumentFormat::Pdf => "pdf",
            DocumentFormat::Custom(name) => name,
        }
    }
}

impl Default for ParsePass {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CompilePass for ParsePass {
    fn name(&self) -> &'static str {
        "parse"
    }

    async fn execute(&mut self, ctx: &mut CompileContext) -> Result<PassResult> {
        let start = Instant::now();

        // Handle pre-parsed input: skip parsing entirely
        if let CompilerInput::PreParsed { nodes, name } = &ctx.input {
            let nodes = nodes.clone();
            let name = name.clone();
            ctx.raw_nodes = nodes;
            ctx.name = name;
            ctx.format = DocumentFormat::Custom("pre-parsed".to_string());
            ctx.metrics.set_nodes_processed(ctx.raw_nodes.len());

            let duration = start.elapsed().as_millis() as u64;
            info!(
                "[parse] Pre-parsed: {} nodes for '{}' ({}ms)",
                ctx.raw_nodes.len(),
                ctx.name,
                duration
            );

            let mut stage_result = PassResult::success("parse");
            stage_result.duration_ms = duration;
            stage_result.metadata.insert(
                "node_count".to_string(),
                serde_json::json!(ctx.raw_nodes.len()),
            );
            stage_result.metadata.insert(
                "source".to_string(),
                serde_json::json!("pre-parsed"),
            );
            return Ok(stage_result);
        }

        // Detect format
        let format = self.detect_format(ctx)?;
        let format_name = Self::format_name(&format).to_string();
        ctx.format = format;

        let input_type = match &ctx.input {
            CompilerInput::File(_) => "file",
            CompilerInput::Content { .. } => "content",
            CompilerInput::Bytes { .. } => "bytes",
            CompilerInput::PreParsed { .. } => unreachable!(),
        };

        info!(
            "[parse] Starting: format={}, input={}, llm={}",
            format_name,
            input_type,
            self.llm_client.is_some()
        );

        // Look up parser in registry
        let parser = self.registry.get(&format_name).ok_or_else(|| {
            vectorless_error::Error::Parse(format!(
                "No parser registered for format '{}'. Available: {:?}",
                format_name,
                self.registry.parser_names()
            ))
        })?;

        // Parse based on input type
        let result = match &ctx.input {
            CompilerInput::File(path) => {
                let path = path.canonicalize().unwrap_or_else(|_| path.clone());
                ctx.source_path = Some(path.clone());
                ctx.name = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("document")
                    .to_string();
                debug!("[parse] Reading file: {:?}", ctx.source_path);
                parser.parse_file(&path).await?
            }
            CompilerInput::Content { content, name, .. } => {
                ctx.name = name.clone();
                debug!("[parse] Parsing inline content ({} chars)", content.len());
                parser.parse_content(content).await?
            }
            CompilerInput::Bytes { data, name, .. } => {
                ctx.name = name.clone();
                debug!("[parse] Parsing bytes ({} bytes)", data.len());
                parser.parse_bytes(data).await?
            }
            CompilerInput::PreParsed { .. } => unreachable!(),
        };

        // Store results
        ctx.raw_nodes = result.nodes;
        ctx.metrics.set_nodes_processed(ctx.raw_nodes.len());

        // Store metadata
        if let Some(page_count) = result.meta.page_count {
            ctx.page_count = Some(page_count);
            debug!("[parse] Document has {} pages", page_count);
        }
        ctx.line_count = Some(result.meta.line_count);

        if let Some(desc) = result.meta.description {
            ctx.description = Some(desc);
        }

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_parse(duration);

        info!(
            "[parse] Complete: {} nodes from '{}' ({}, {}ms)",
            ctx.raw_nodes.len(),
            ctx.name,
            format_name,
            duration
        );

        let mut stage_result = PassResult::success("parse");
        stage_result.duration_ms = duration;
        stage_result.metadata.insert(
            "node_count".to_string(),
            serde_json::json!(ctx.raw_nodes.len()),
        );
        stage_result
            .metadata
            .insert("format".to_string(), serde_json::json!(&format_name));

        Ok(stage_result)
    }
}
