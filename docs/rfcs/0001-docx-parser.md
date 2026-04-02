# DOCX Parser Implementation Plan

## Overview

Add DOCX (Microsoft Word) document parsing support to Vectorless, enabling hierarchical tree-based retrieval for Word documents.

## DOCX File Structure

A DOCX file is a ZIP archive containing XML files:

```
document.docx
├── [Content_Types].xml      # MIME type definitions
├── _rels/.rels              # Package relationships
├── word/
│   ├── document.xml         # Main content (paragraphs, tables)
│   ├── styles.xml           # Style definitions
│   ├── numbering.xml        # List numbering (optional)
│   ├── core.xml             # Metadata (title, author)
│   └── _rels/document.xml.rels
```

**Key file**: `word/document.xml` contains all paragraphs with style references.

## Architecture

### Module Structure

```
src/document/
├── mod.rs           # Export docx module
├── docx/
│   ├── mod.rs       # Module exports
│   ├── parser.rs    # Main parser implementation
│   ├── styles.rs    # Style resolution (heading detection)
│   └── types.rs     # DOCX-specific types
```

### Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
zip = "2.2"           # ZIP archive handling
roxmltree = "0.20"    # Fast XML parsing (read-only)
```

## Implementation Details

### 1. Types (`types.rs`)

```rust
/// Parsed DOCX paragraph.
pub struct DocxParagraph {
    /// Text content.
    pub text: String,
    /// Style ID (e.g., "Heading1", "Normal").
    pub style_id: Option<String>,
    /// Detected heading level (1-6), None for body text.
    pub heading_level: Option<u8>,
    /// List item info (if part of a list).
    pub list_info: Option<ListInfo>,
}

/// List item information.
pub struct ListInfo {
    /// Nesting level (0 = top level).
    pub level: u8,
    /// Whether it's an ordered list.
    pub ordered: bool,
}

/// Parsed style definition.
pub struct DocxStyle {
    pub style_id: String,
    pub name: String,
    pub is_heading: bool,
    pub heading_level: Option<u8>,
}
```

### 2. Style Resolution (`styles.rs`)

Heading detection strategy (in priority order):

1. **Built-in styles**: `Heading1` → `Heading6` (most common)
2. **Custom heading styles**: Match by name pattern `/heading\s*(\d)/i`
3. **Outline level**: Read `<w:outlineLvl>` from style definition
4. **Heuristics**: Bold + larger font + short text → potential heading

```rust
pub struct StyleResolver {
    /// Map from style_id to resolved style info.
    styles: HashMap<String, DocxStyle>,
}

impl StyleResolver {
    /// Parse styles.xml and build resolver.
    pub fn from_xml(styles_xml: &str) -> Self;

    /// Get heading level for a style ID.
    pub fn get_heading_level(&self, style_id: &Option<String>) -> Option<u8>;

    /// Check if style is a heading.
    pub fn is_heading(&self, style_id: &Option<String>) -> bool;
}
```

### 3. Parser (`parser.rs`)

```rust
pub struct DocxParser;

impl DocumentParser for DocxParser {
    fn parse(&self, content: &[u8]) -> Result<ParseResult> {
        // 1. Parse ZIP archive
        let archive = ZipArchive::new(Cursor::new(content))?;

        // 2. Read styles.xml (optional, may not exist)
        let style_resolver = Self::parse_styles(&archive)?;

        // 3. Read document.xml
        let document_xml = Self::read_file(&archive, "word/document.xml")?;
        let root = roxmltree::Document::parse(&document_xml)?;

        // 4. Traverse paragraphs
        let paragraphs = Self::parse_paragraphs(&root, &style_resolver)?;

        // 5. Convert to RawNodes
        let raw_nodes = Self::build_raw_nodes(paragraphs)?;

        Ok(ParseResult { nodes: raw_nodes })
    }

    fn format(&self) -> DocumentFormat {
        DocumentFormat::Docx
    }
}
```

### 4. Parsing Flow

```
┌─────────────────────────────────────────────────────────────┐
│                     DOCX File (.docx)                        │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  1. Unzip                                                    │
│     - word/document.xml                                      │
│     - word/styles.xml (optional)                             │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  2. Parse styles.xml                                         │
│     - Build StyleResolver                                    │
│     - Map style_id → heading_level                           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  3. Parse document.xml                                       │
│     - Find all <w:p> elements (paragraphs)                   │
│     - Extract text from <w:t> elements                       │
│     - Get style from <w:pStyle val="..."/>                   │
│     - Resolve heading_level via StyleResolver                │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  4. Build RawNodes                                           │
│     - Heading → new section (parent)                         │
│     - Body text → append to current section                  │
│     - Track heading hierarchy for nesting                    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  5. Return ParseResult { nodes: Vec<RawNode> }               │
└─────────────────────────────────────────────────────────────┘
```

### 5. XML Structure Reference

**document.xml** structure:

```xml
<w:document>
  <w:body>
    <w:p>                              <!-- Paragraph -->
      <w:pPr>                          <!-- Paragraph properties -->
        <w:pStyle w:val="Heading1"/>   <!-- Style reference -->
      </w:pPr>
      <w:r>                            <!-- Text run -->
        <w:t>Chapter Title</w:t>       <!-- Actual text -->
      </w:r>
    </w:p>
    <w:p>
      <w:r>
        <w:t>Body text content...</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>
```

**styles.xml** structure:

```xml
<w:styles>
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:pPr>
      <w:outlineLvl w:val="0"/>    <!-- Outline level 0 = H1 -->
    </w:pPr>
  </w:style>
</w:styles>
```

### 6. RawNode Building Strategy

```rust
fn build_raw_nodes(paragraphs: Vec<DocxParagraph>) -> Result<Vec<RawNode>> {
    let mut nodes = Vec::new();
    let mut current_node: Option<RawNode> = None;
    let mut heading_stack: Vec<(u8, RawNode)> = Vec::new();  // (level, node)

    for para in paragraphs {
        if para.text.is_empty() {
            continue;
        }

        if let Some(level) = para.heading_level {
            // Save previous node
            if let Some(node) = current_node.take() {
                nodes.push(node);
            }

            // Handle heading hierarchy
            // - Pop stack until we find parent level
            // - Create new section node
            let node = RawNode {
                title: para.text.clone(),
                content: String::new(),
                children: Vec::new(),
            };

            heading_stack.retain(|(l, _)| *l < level);
            heading_stack.push((level, node));
        } else {
            // Body text - append to current section
            if let Some(ref mut node) = current_node {
                if !node.content.is_empty() {
                    node.content.push('\n');
                }
                node.content.push_str(&para.text);
            }
        }
    }

    // Don't forget the last node
    if let Some(node) = current_node {
        nodes.push(node);
    }

    // TODO: Handle heading_stack to build proper hierarchy

    Ok(nodes)
}
```

### 7. Edge Cases

| Case | Handling |
|------|----------|
| **No styles.xml** | Use heuristics (bold + font size) or treat all as body |
| **Empty paragraphs** | Skip |
| **Tables** | Extract as formatted text (for now) |
| **Images** | Ignore (no text content) |
| **Nested lists** | Track list level from numbering.xml |
| **Mixed content** | Handle runs with different formatting |

### 8. Testing

Create test fixtures:

```
tests/fixtures/
├── simple.docx           # Basic headings + paragraphs
├── nested.docx           # H1 → H2 → H3 hierarchy
├── no_styles.docx        # Document without styles.xml
├── tables.docx           # Contains tables
└── lists.docx            # Contains numbered/bulleted lists
```

Unit tests:

```rust
#[test]
fn test_parse_simple_docx() {
    let content = include_bytes!("../fixtures/simple.docx");
    let parser = DocxParser;
    let result = parser.parse(content).unwrap();
    assert!(!result.nodes.is_empty());
}

#[test]
fn test_heading_detection() {
    let resolver = StyleResolver::from_xml(STYLES_XML);
    assert_eq!(resolver.get_heading_level(&Some("Heading1".into())), Some(1));
    assert_eq!(resolver.get_heading_level(&Some("Normal".into())), None);
}
```

## Integration

### 1. Update `src/document/mod.rs`

```rust
pub mod docx;
pub use docx::DocxParser;
```

### 2. Register in `ParserRegistry`

```rust
registry.register(DocumentFormat::Docx, Box::new(DocxParser));
```

### 3. Update `DocumentFormat` enum

```rust
pub enum DocumentFormat {
    Markdown,
    Pdf,
    Docx,  // Add this
}
```

### 4. Update client API

```rust
// Auto-detect format from file extension
pub fn detect_format(path: &Path) -> DocumentFormat {
    match path.extension().and_then(|s| s.to_str()) {
        Some("md") => DocumentFormat::Markdown,
        Some("pdf") => DocumentFormat::Pdf,
        Some("docx") => DocumentFormat::Docx,  // Add this
        _ => DocumentFormat::Markdown,
    }
}
```

## Effort Estimate

| Task | Time |
|------|------|
| Types & structures | 1 hour |
| Style resolution | 2 hours |
| Main parser | 3 hours |
| RawNode building | 2 hours |
| Edge cases | 2 hours |
| Testing | 2 hours |
| **Total** | **~12 hours (1.5 days)** |

## Future Enhancements (Out of Scope)

- [ ] Table parsing with structure preservation
- [ ] List nesting from numbering.xml
- [ ] Header/footer extraction
- [ ] Comments and annotations
- [ ] Tracked changes (revisions)
- [ ] Embedded objects

## References

- [ECMA-376: Office Open XML](https://www.ecma-international.org/publications-and-standards/standards/ecma-376/)
- [DOCX file format specification](https://docs.microsoft.com/en-us/openspecs/office_standards/ms-docx/)
