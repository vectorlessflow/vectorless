# Advanced Example - Full Configuration

Use a configuration file for fine-grained control.

## Setup

```bash
pip install vectorless

# Copy the example config
cp ../../../config.toml ./vectorless.toml

# Edit to customize your settings
vim vectorless.toml
```

## Run

```bash
python main.py
```

## Configuration File Structure

```toml
[llm]
api_key = "sk-..."

[llm.summary]
model = "gpt-4o-mini"
max_tokens = 200

[llm.retrieval]
model = "gpt-4o"
max_tokens = 100

[retrieval]
top_k = 5
beam_width = 3
max_iterations = 10

[storage]
workspace_dir = "./workspace"
cache_size = 100
```
