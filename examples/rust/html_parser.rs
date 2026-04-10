// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! HTML Parser Example.
//!
//! This example demonstrates how to parse HTML documents using vectorless.
//!
//! # Features
//!
//! - Parses HTML5 documents
//! - Extracts heading hierarchy (h1-h6)
//! - Extracts content from paragraphs, lists, tables
//! - Extracts metadata from <head> (title, description, etc.)
//!
//! # Usage
//!
//! ```bash
//! cargo run --example html_parser
//! ```

use vectorless::parser::{DocumentParser, HtmlConfig, HtmlParser};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== HTML Parser Example ===\n");

    // 1. Basic HTML parsing
    println!("--- Step 1: Basic HTML Parsing ---\n");
    demo_basic_parsing().await?;

    // 2. Parsing with metadata
    println!("\n--- Step 2: HTML with Metadata ---\n");
    demo_metadata_parsing().await?;

    // 3. Complex HTML structure
    println!("\n--- Step 3: Complex HTML Structure ---\n");
    demo_complex_structure().await?;

    // 4. Configuration options
    println!("\n--- Step 4: Configuration Options ---\n");
    demo_configuration().await?;

    // 5. Integration with Engine
    println!("\n--- Step 5: Integration with Engine ---\n");
    demo_engine_integration();

    println!("\n=== Done ===");
    Ok(())
}

/// Demonstrate basic HTML parsing.
async fn demo_basic_parsing() -> vectorless::Result<()> {
    let parser = HtmlParser::new();
    let html = r#"
<!DOCTYPE html>
<html>
<head><title>Basic Document</title></head>
<body>
    <h1>Main Title</h1>
    <p>This is the introduction paragraph.</p>

    <h2>Section 1</h2>
    <p>Content for section 1.</p>

    <h2>Section 2</h2>
    <p>Content for section 2.</p>
    <h3>Subsection 2.1</h3>
    <p>Detailed content here.</p>
</body>
</html>
"#;

    let result = parser.parse(html).await?;

    println!("Document: {}", result.meta.name);
    println!("Nodes extracted: {}\n", result.nodes.len());

    for node in &result.nodes {
        println!("  {} {} (level {})",
            "•".repeat(node.level),
            node.title,
            node.level
        );
        if !node.content.is_empty() {
            let preview: String = node.content.chars().take(50).collect();
            println!("    Content: {}...", preview);
        }
    }

    Ok(())
}

/// Demonstrate parsing HTML with metadata.
async fn demo_metadata_parsing() -> vectorless::Result<()> {
    let parser = HtmlParser::new();
    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Technical Documentation</title>
    <meta name="description" content="Complete guide to the API">
    <meta name="author" content="Documentation Team">
    <meta name="keywords" content="API, REST, documentation">
    <meta property="og:description" content="Open Graph description">
</head>
<body>
    <h1>API Reference</h1>
    <p>Introduction to the API.</p>
</body>
</html>
"#;

    let result = parser.parse(html).await?;

    println!("Metadata extracted:");
    println!("  Title: {}", result.meta.name);
    println!("  Description: {:?}", result.meta.description);
    println!("  Format: {:?}", result.meta.format);
    println!("  Lines: {}", result.meta.line_count);

    Ok(())
}

/// Demonstrate parsing complex HTML structure.
async fn demo_complex_structure() -> vectorless::Result<()> {
    let parser = HtmlParser::new();
    let html = r#"
<!DOCTYPE html>
<html>
<body>
    <h1>Complex Document</h1>

    <h2>Lists</h2>
    <ul>
        <li>First item</li>
        <li>Second item</li>
        <li>Third item</li>
    </ul>

    <ol>
        <li>Step one</li>
        <li>Step two</li>
        <li>Step three</li>
    </ol>

    <h2>Table</h2>
    <table>
        <tr><th>Name</th><th>Value</th></tr>
        <tr><td>Option A</td><td>100</td></tr>
        <tr><td>Option B</td><td>200</td></tr>
    </table>

    <h2>Code Block</h2>
    <pre><code>fn main() {
    println!("Hello, World!");
}</code></pre>

    <h2>Blockquote</h2>
    <blockquote>
        This is a quoted text from another source.
        It can span multiple lines.
    </blockquote>
</body>
</html>
"#;

    let result = parser.parse(html).await?;

    println!("Nodes with complex content:\n");
    for node in &result.nodes {
        println!("  [Level {}] {}", node.level, node.title);
        if node.content.contains("•") || node.content.contains("1.") {
            println!("    → Contains list content");
        }
        if node.content.contains("|") {
            println!("    → Contains table content");
        }
        if node.content.contains("```") {
            println!("    → Contains code block");
        }
        if node.content.contains(">") {
            println!("    → Contains blockquote");
        }
    }

    Ok(())
}

/// Demonstrate configuration options.
async fn demo_configuration() -> vectorless::Result<()> {
    // Default configuration
    let _default_parser = HtmlParser::new();
    println!("Default config:");
    println!("  - max_heading_level: 6");
    println!("  - include_code_blocks: true");
    println!("  - merge_small_nodes: true");
    println!("  - min_content_length: 50\n");

    // Custom configuration
    let config = HtmlConfig::new()
        .with_max_heading_level(3)  // Only h1-h3
        .with_code_blocks(false)     // Exclude code
        .with_min_content_length(20) // Smaller threshold
        .with_default_title("Overview");

    let custom_parser = HtmlParser::with_config(config);
    println!("Custom config:");
    println!("  - max_heading_level: 3");
    println!("  - include_code_blocks: false");
    println!("  - min_content_length: 20");
    println!("  - default_title: \"Overview\"\n");

    // Parse with custom config
    let html = r#"
<html>
<body>
    <h1>Title</h1>
    <p>Short.</p>
    <h4>This heading is ignored (level > 3)</h4>
    <p>This content goes to parent.</p>
</body>
</html>
"#;

    let result = custom_parser.parse(html).await?;
    println!("Nodes with max_level=3: {}", result.nodes.len());

    // Show preset configs
    println!("\nPreset configurations:");
    let simple = HtmlConfig::simple();
    println!("  HtmlConfig::simple():");
    println!("    - merge_small_nodes: {}", simple.merge_small_nodes);
    println!("    - min_content_length: {}", simple.min_content_length);

    let no_code = HtmlConfig::no_code_blocks();
    println!("  HtmlConfig::no_code_blocks():");
    println!("    - include_code_blocks: {}", no_code.include_code_blocks);

    Ok(())
}

/// Demonstrate integration with Engine.
fn demo_engine_integration() {
    println!("Integration with Engine:\n");

    println!("```rust");
    println!("use vectorless::{{EngineBuilder, IndexContext, QueryContext}};");
    println!("use vectorless::parser::DocumentFormat;");
    println!();
    println!("# #[tokio::main]");
    println!("# async fn main() -> vectorless::Result<()> {{");
    println!("    let engine = EngineBuilder::new()");
    println!("        .with_workspace(\"./workspace\")");
    println!("        .build()");
    println!("        .await?;");
    println!();
    println!("    // Method 1: From HTML file");
    println!("    let result = engine.index(");
    println!("        IndexContext::from_path(\"./documentation.html\")");
    println!("    ).await?;");
    println!("    let doc_id = result.doc_id().unwrap().to_string();");
    println!();
    println!("    // Method 2: From HTML content");
    println!("    let html = r#\"");
    println!("<html>");
    println!("<head><title>My Doc</title></head>");
    println!("<body>");
    println!("    <h1>Introduction</h1>");
    println!("    <p>Content here...</p>");
    println!("</body>");
    println!("</html>");
    println!("\"#;");
    println!();
    println!("    let result = engine.index(");
    println!("        IndexContext::from_content(html, DocumentFormat::Html)");
    println!("            .with_name(\"my-document\")");
    println!("    ).await?;");
    println!("    let doc_id = result.doc_id().unwrap().to_string();");
    println!();
    println!("    // Query the indexed document");
    println!("    let result = engine.query(");
    println!("        QueryContext::new(\"What is the introduction?\").with_doc_id(&doc_id)");
    println!("    ).await?;");
    println!("    println!(\"{{}}\", result.content);");
    println!();
    println!("    Ok(())");
    println!("}}");
    println!("```\n");

    println!("Supported file extensions:");
    println!("  - .html, .htm → HTML format");
    println!("  - .md, .markdown → Markdown format");
    println!("  - .pdf → PDF format");
    println!("  - .docx → Word document");
}
