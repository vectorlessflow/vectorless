<h1>Vectorless</h1>

[![PyPI](https://img.shields.io/pypi/v/vectorless.svg)](https://pypi.org/project/vectorless/)
[![PyPI Downloads](https://static.pepy.tech/badge/vectorless/month)](https://pepy.tech/projects/vectorless)

<p>Knowing by reasoning, not vectors.</p>
<p>Deep and reliable. Vectorless plays nicely with your documents. Ask questions in plain language; get answers by reasoning with Vectorless.</p>

## Installation

Install using `pip install -U vectorless`. For more details, see the [Installation](https://vectorless.dev) section in the documentation.

## A Simple Example

```python
import asyncio
from vectorless import Engine

async def main():
    engine = Engine(api_key="sk-...", model="gpt-4o", endpoint="https://api.openai.com/v1")

    # Compile a document
    result = await engine.compile(path="./report.pdf")
    doc_id = result.doc_id

    # Ask a question
    response = await engine.ask("What is the total revenue?", doc_ids=[doc_id])
    print(response.single().content)

asyncio.run(main())
```

## Help

See [documentation](https://www.vectorless.dev/docs/intro) for more details.


## Contributing

Contributions welcome! See [Contributing](CONTRIBUTING.md) for setup and guidelines.

## License

Apache License 2.0
