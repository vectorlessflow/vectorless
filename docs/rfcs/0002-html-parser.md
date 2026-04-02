# RFC-0002: HTML Parser Implementation

**Status**: Proposed

## Summary

Add HTML document parsing support to Vectorless, enabling hierarchical tree-based retrieval for web pages and HTML documents.

## Motivation

HTML is one of the most common document formats:
- Web scraping and content extraction
- Documentation websites
- Blog posts and articles
- Technical documentation

Unlike Markdown/PDF/DOCX, HTML documents often contain:
- Navigation menus
- Sidebars
- Footers
- Advertisements
- Scripts and styles

The challenge is extracting **meaningful content** while filtering noise.

## HTML Structure Analysis

### Content Structure

```html
<!DOCTYPE html>
<html>
<head>
    <title>Document Title</title>
    <meta name="description" content="...">
</head>
<body>
    <nav>...</nav>           <!-- Navigation - usually skip -->
    <aside>...</aside>       <!-- Sidebar - usually skip -->
    <main>
        <article>
            <h1>Main Title</h1>
            <section>
                <h2>Section 1</h2>
                <p>Content...</p>
            </section>
            <section>
                <h2>Section 2</h2>
                <p>Content...</p>
                <h3>Subsection</h3>
                <p>More content...</p>
            </section>
        </article>
    </main>
    <footer>...</footer>     <!-- Footer - usually skip -->
</body>
</html>
```

### Heading Hierarchy

HTML has explicit heading tags:
- `<h1>` - `<h6>` : Heading levels 1-6
- `<title>` : Document title
- `<figcaption>` : Figure captions (optional heading)

### Semantic Elements (HTML5)

| Element | Meaning | Use for TOC? |
|---------|---------|--------------|
| `<article>` | Self-contained content | Yes - content boundary |
| `<section>` | Thematic grouping | Yes - section boundary |
| `<main>` | Main content area | Yes - skip nav/sidebar |
| `<nav>` | Navigation links | No - skip |
| `<aside>` | Sidebar content | No - skip |
| `<header>` | Page header | No - skip |
| `<footer>` | Page footer | No - skip |

## Proposed Solution

### Module Structure

```
src/document/html/
├── mod.rs           # Module exports
├── parser.rs        # Main parser implementation
├── extractor.rs     # Content extraction (readability)
└── types.rs         # HTML-specific types
```

### Dependencies

```toml
# HTML parsing
scraper = "0.22"     # HTML parsing (CSS selectors)
```

Alternative: `tl` (faster, no CSS selectors) or `html5ever` (spec-compliant)

### Types

```rust
/// HTML parser configuration.
pub struct HtmlConfig {
    /// Skip navigation elements.
    pub skip_nav: bool,

    /// Skip aside/sidebar elements.
    pub skip_aside: bool,

    /// Skip footer elements.
    pub skip_footer: bool,

    /// Extract main content only (using readability algorithm).
    pub extract_main_content: bool,

    /// Maximum heading level to parse (1-6).
    pub max_heading_level: usize,
}

/// Parsed HTML element.
pub struct HtmlElement {
    /// Text content.
    pub text: String,
    /// Tag name (h1-h6, p, etc.).
    pub tag: String,
    /// Heading level (1-6), if applicable.
    pub heading_level: Option<u8>,
}
```

### Parser Flow

```
┌─────────────────────────────────────────────────────────────┐
│                     HTML File (.html)                        │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  1. Parse HTML                                               │
│     - Use scraper to build DOM tree                          │
│     - Handle malformed HTML gracefully                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  2. Extract Main Content (optional)                          │
│     - Find <main> or <article> element                       │
│     - Skip <nav>, <aside>, <footer>                          │
│     - Or use readability algorithm for complex pages         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  3. Extract Heading Structure                                │
│     - Find all <h1>-<h6> elements                            │
│     - Build heading hierarchy                                │
│     - Extract text content between headings                  │
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

### Content Extraction Strategy

**Level 1: Semantic HTML5** (Simple, Fast)

```rust
fn extract_main_content(&self, doc: &Html) -> ElementRef {
    // Priority: <main> > <article> > <body>
    if let Some(main) = doc.select(&selector("main")).next() {
        return main;
    }
    if let Some(article) = doc.select(&selector("article")).next() {
        return article;
    }
    doc.select(&selector("body")).next().unwrap()
}
```

**Level 2: Skip Known Noise** (Medium)

```rust
const SKIP_TAGS: &[&str] = &["nav", "aside", "footer", "script", "style", "noscript"];

fn should_skip(&self, elem: &ElementRef) -> bool {
    SKIP_TAGS.contains(&elem.value().name())
}
```

**Level 3: Readability Algorithm** (Advanced, Optional)

For complex web pages without semantic structure, implement a simplified readability:
- Calculate text density
- Find largest text block
- Remove low-density regions

This is more complex and can be added later as enhancement.

### Implementation Details

```rust
pub struct HtmlParser {
    config: HtmlConfig,
}

impl HtmlParser {
    /// Parse HTML content and extract nodes.
    fn extract_nodes(&self, html: &str) -> Vec<RawNode> {
        let doc = Html::parse_document(html);

        // 1. Find main content area
        let root = self.find_main_content(&doc);

        // 2. Extract all heading and text elements
        let elements = self.extract_elements(&root);

        // 3. Build nodes from elements
        self.build_raw_nodes(elements)
    }

    /// Find the main content area.
    fn find_main_content<'a>(&self, doc: &'a Html) -> ElementRef<'a> {
        // Try <main> first
        if let Some(main) = doc.select(&selector("main")).next() {
            return main;
        }

        // Try <article>
        if let Some(article) = doc.select(&selector("article")).next() {
            return article;
        }

        // Fallback to <body>
        doc.select(&selector("body"))
            .next()
            .expect("HTML must have body")
    }

    /// Extract elements from the content area.
    fn extract_elements(&self, root: &ElementRef) -> Vec<HtmlElement> {
        let mut elements = Vec::new();

        for node in root.descendants() {
            if let Some(elem) = node.value().as_element() {
                let tag = elem.name();

                // Check if it's a heading
                if let Some(level) = self.get_heading_level(tag) {
                    let text = node.text().collect::<String>();
                    if !text.trim().is_empty() {
                        elements.push(HtmlElement {
                            text: text.trim().to_string(),
                            tag: tag.to_string(),
                            heading_level: Some(level),
                        });
                    }
                }
            }
        }

        elements
    }

    /// Get heading level from tag name.
    fn get_heading_level(&self, tag: &str) -> Option<u8> {
        match tag {
            "h1" => Some(1),
            "h2" => Some(2),
            "h3" => Some(3),
            "h4" => Some(4),
            "h5" => Some(5),
            "h6" => Some(6),
            _ => None,
        }
    }
}
```

### Edge Cases

| Case | Handling |
|------|----------|
| **Malformed HTML** | scraper handles gracefully |
| **No headings** | Create single node with all text |
| **No semantic elements** | Use entire body |
| **Nested articles** | Use first/deepest article |
| **Multiple h1 tags** | Treat each as level 1 heading |
| **Scripts/styles** | Skip by default |
| **Tables** | Extract text, ignore structure (for now) |
| **Images** | Extract alt text only |

### Testing Strategy

Create test fixtures:

```
tests/fixtures/
├── simple.html          # Basic h1-h6 structure
├── semantic.html        # With <main>, <article>, <section>
├── noisy.html           # With nav, aside, footer
├── no_headings.html     # Just paragraphs
└── malformed.html       # Broken HTML
```

## Effort Estimate

| Task | Time |
|------|------|
| Types & configuration | 1 hour |
| Main parser | 2 hours |
| Content extraction | 2 hours |
| Edge cases | 1 hour |
| Testing | 2 hours |
| **Total** | **~8 hours (1 day)** |

## Future Enhancements (Out of Scope)

- [ ] Readability algorithm for content extraction
- [ ] Table structure preservation
- [ ] Code block detection (`<pre><code>`)
- [ ] Link extraction and following
- [ ] Meta description extraction
- [ ] Language detection

## Comparison with Alternatives

| Approach | Pros | Cons |
|----------|------|------|
| **scraper** (proposed) | CSS selectors, mature | Slower than tl |
| **tl** | Very fast | No CSS selectors |
| **html5ever** | Spec-compliant | More complex API |
| **readability-rs** | Smart extraction | External dependency |

## References

- [HTML5 Semantic Elements](https://developer.mozilla.org/en-US/docs/Glossary/Semantics#semantics_in_html)
- [scraper crate](https://docs.rs/scraper/)
- [Readability algorithm](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/tabs/saveAsPDF)
