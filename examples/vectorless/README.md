# Single-Document Reasoning Challenge

A demonstration of Vectorless's ability to perform deep reasoning on complex technical documents.

## Overview

This project compiles a realistic quantum computing research report and asks questions that require:
- Cross-referencing information across distant sections
- Tracing dependency chains between different entities
- Extracting details buried in nested structures
- Multi-step reasoning beyond simple keyword matching

## Installation

Requires the Vectorless Python SDK:

```bash
pip install vectorless
```

## Usage

Set your LLM API credentials and run:

```bash
LLM_API_KEY=sk-xxx LLM_MODEL=gpt-4o LLM_ENDPOINT=https://api.openai.com/v1 python main.py
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `LLM_API_KEY` | Your LLM provider API key | `sk-...` |
| `LLM_MODEL` | Model identifier | `gpt-4o` |
| `LLM_ENDPOINT` | API endpoint URL | `https://api.openai.com/v1` |

## Challenge Questions

1. **Refrigerator cost & location** — Connects Lab B's characterization requirements with Lab A's equipment specs and capital expenditure data

2. **Materials dependency** — Traces how Lab C's error correction milestone depends on Lab A's materials science improvement

3. **Firmware bug impact** — Calculates affected qubits by connecting Lab D's incident report with Lab A's hardware configuration

4. **Gap to target** — Computes the difference between current achievement and future goals using derived values

5. **Revenue coverage** — Evaluates whether a single revenue source can cover projected capital needs

## License

Apache-2.0
