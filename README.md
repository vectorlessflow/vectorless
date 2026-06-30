<h1>Vectorless</h1>

[![PyPI](https://img.shields.io/pypi/v/vectorless.svg)](https://pypi.org/project/vectorless/)
[![PyPI Downloads](https://static.pepy.tech/badge/vectorless/month)](https://pepy.tech/projects/vectorless)

<p>Knowing by reasoning, not vectors.</p>
<p>Deep and reliable. Vectorless plays nicely with your documents. Ask questions in plain language; Get answers by reasoning with Vectorless.</p>

> [!WARNING]
> **Experimental project — not production-ready.** Vectorless is under active, rapid development. Releases are cut on an irregular schedule and may introduce **breaking API changes without notice**. It is currently positioned as a research/experimental project and makes no stability or production-readiness guarantees. Pin a version and expect churn.

## Installation

Install using `pip install -U vectorless`. For more details, see the [Installation](https://vectorless.dev/docs/installation) section in the documentation.

## A Simple Example

```python
import asyncio
from vectorless import Engine

async def main():
    async with Engine(api_key="sk-...", model="gpt-4o") as engine:
        # Compile a document
        doc = await engine.compile(path="./report.pdf")

        # Ask a question
        response = await engine.ask("What is the total revenue?", doc_ids=[doc.doc_id])
        print(response.answer)

asyncio.run(main())
```

## Help

See [documentation](https://vectorless.dev/docs/getting-started) for more details.


## Contributing

Contributions welcome! See [Contributing](CONTRIBUTING.md) for setup and guidelines.

## License

Apache License 2.0
