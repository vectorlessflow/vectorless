# IndexMetrics Example

Demonstrates how to inspect detailed indexing pipeline metrics via `IndexMetrics`.

`IndexMetrics` is attached to each `IndexItem` and provides:

| Field                  | Description                                  |
|------------------------|----------------------------------------------|
| `total_time_ms`        | Total indexing time                           |
| `parse_time_ms`        | Document parsing stage duration               |
| `build_time_ms`        | Tree building stage duration                  |
| `enhance_time_ms`      | Summary/enhancement stage duration            |
| `nodes_processed`      | Number of tree nodes processed                |
| `summaries_generated`  | Successfully generated summaries              |
| `summaries_failed`     | Failed summary generations                    |
| `llm_calls`            | Total LLM API calls made                      |
| `total_tokens_generated` | Total tokens produced by the LLM            |
| `topics_indexed`       | Topics added to the reasoning index           |
| `keywords_indexed`     | Keywords added to the reasoning index         |

This example compares documents indexed with and without summaries enabled
to show how `IndexOptions` affect pipeline stages and LLM usage.

## Setup

```bash
pip install vectorless
```

## Run

```bash
python main.py
```

## Environment Variables

| Variable                | Description          | Default   |
|------------------------|----------------------|-----------|
| `VECTORLESS_API_KEY`   | LLM API key          | `sk-...`  |
| `VECTORLESS_MODEL`     | LLM model name       | `gpt-4o`  |
| `VECTORLESS_ENDPOINT`  | Custom API endpoint  | `None`    |
