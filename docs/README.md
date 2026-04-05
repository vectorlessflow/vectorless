# Vectorless Documentation

Welcome to the Vectorless documentation.

## What is Vectorless?

Vectorless is a **reasoning-native document intelligence engine** that uses LLM-powered tree navigation instead of vector embeddings. It preserves document structure and uses intelligent navigation to find relevant content.

## Key Features

- **Dual Pipeline Architecture** - Separate Index and Retrieval pipelines
- **Pilot System** - LLM-guided navigation with layered fallback
- **Multi-Strategy Retrieval** - Keyword, LLM, and Structure-aware strategies
- **Zero Infrastructure** - No vector database, no embeddings
- **Multi-Format Support** - Markdown, PDF, DOCX, HTML

## Getting Started

- [Quick Start Guide](guides/quick-start.md) - Get up and running in 5 minutes

## Guides

| Guide | Description |
|-------|-------------|
| [Quick Start](guides/quick-start.md) | Get up and running quickly |
| [Dual Pipeline](guides/dual-pipeline.md) | Understand Index + Retrieval pipelines |
| [Pilot System](guides/pilot-system.md) | LLM-guided navigation |
| [Multi-Strategy Retrieval](guides/multi-strategy.md) | Keyword, LLM, Structure strategies |

## Design Documents

System architecture and core mechanism documentation.

| Document | Description |
|----------|-------------|
| [pilot.md](design/pilot.md) | Pilot system design |
| [content-aggregation.md](design/content-aggregation.md) | Content aggregation design |
| [client-module.md](design/client-module.md) | Client API design |
| [v3.md](design/v3.md) | Version 3 architecture |

## RFCs (Feature Proposals)

Detailed design documents for new features.

| RFC | Title | Status |
|-----|-------|--------|
| [0001](rfcs/0001-docx-parser.md) | DOCX Parser | Implemented |
| [0002](rfcs/0002-html-parser.md) | HTML Parser | Implemented |

### RFC Process

1. Create `rfcs/0XXX-feature-name.md` using the [template](rfcs/template.md)
2. Discuss and refine the design
3. Once approved, implement and update status to "Implemented"
