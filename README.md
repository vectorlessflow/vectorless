<div align="center">

<img src="https://vectorless.dev/img/with-title.png" alt="Vectorless" width="400">

<h1>Document Engine for AI</h1>

[![PyPI](https://img.shields.io/pypi/v/vectorless.svg)](https://pypi.org/project/vectorless/)
[![PyPI Downloads](https://static.pepy.tech/badge/vectorless/month)](https://pepy.tech/projects/vectorless)
[![Crates.io](https://img.shields.io/crates/v/vectorless.svg)](https://crates.io/crates/vectorless)
[![Crates.io Downloads](https://img.shields.io/crates/d/vectorless.svg)](https://crates.io/crates/vectorless)
[![Docs](https://docs.rs/vectorless/badge.svg)](https://docs.rs/vectorless)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

</div>

**Vectorless** is a reasoning-native document engine with the core written in Rust. It will reason through any of your structured documents — **PDFs, Markdown, reports, contracts** — and retrieve only what's relevant. Nothing more, nothing less.

- **Reason, don't vector.** — Retrieval is guided by reasoning over document structure.
- **Model fails, we fail.** — No silent degradation. No heuristic fallbacks.
- **No thought, no answer.** — Only LLM-reasoned output counts as an answer.


## Quick Start

```bash
pip install vectorless
```

```python
import asyncio
from vectorless import Engine, IndexContext, QueryContext

async def main():
    engine = Engine(api_key="sk-...", model="gpt-4o", endpoint="https://api.openai.com/v1")

    # Index a document
    result = await engine.index(IndexContext.from_path("./report.pdf"))
    doc_id = result.doc_id

    # Query
    result = await engine.query(
        QueryContext("What is the total revenue?").with_doc_ids([doc_id])
    )
    print(result.single().content)

asyncio.run(main())
```

## What It's For

Vectorless is designed for applications that need **precise** document retrieval:

- **Financial analysis** — Extract specific figures from reports, compare across filings
- **Legal research** — Find relevant clauses, trace definitions across documents
- **Technical documentation** — Navigate large manuals, locate specific procedures
- **Academic research** — Cross-reference findings across papers
- **Compliance** — Audit trails with source references for every answer

## Examples

See [examples/](examples/) for complete usage patterns.

## Contributing

Contributions welcome! If you find this useful, please ⭐ the repo — it helps others discover it.

## Star History

<a href="https://www.star-history.com/?repos=vectorlessflow%2Fvectorless&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=vectorlessflow/vectorless&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=vectorlessflow/vectorless&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=vectorlessflow/vectorless&type=date&legend=top-left" />
 </picture>
</a>

## License

Apache License 2.0
