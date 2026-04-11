# Configuration Reference

Vectorless supports multiple configuration layers. Settings from higher-priority sources override lower ones.

## Priority (highest → lowest)

1. Builder/constructor parameters
2. Environment variables
3. Explicit config file (`config_path`)
4. Auto-detected config file (`vectorless.toml`, `config.toml`, `.vectorless.toml`)
5. Default values

## Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `OPENAI_API_KEY` | LLM API key | `sk-...` |
| `VECTORLESS_MODEL` | Default model | `gpt-4o-mini` |
| `VECTORLESS_ENDPOINT` | Custom API endpoint | `https://api.openai.com/v1` |
| `VECTORLESS_WORKSPACE` | Workspace directory | `./data` |

## Configuration File

Create `vectorless.toml` for full control:

```bash
cp vectorless.example.toml ./vectorless.toml
```

### LLM Configuration

Three separate LLM clients for different tasks:

```toml
[llm]
api_key = "sk-your-api-key"

# Summarization — used during indexing
[llm.summary]
model = "gpt-4o-mini"
endpoint = "https://api.openai.com/v1"
max_tokens = 200
temperature = 0.0

# Retrieval — used during query analysis
[llm.retrieval]
model = "gpt-4o"
endpoint = "https://api.openai.com/v1"
max_tokens = 100
temperature = 0.0

# Pilot — used for tree navigation guidance
[llm.pilot]
model = "gpt-4o-mini"
endpoint = "https://api.openai.com/v1"
max_tokens = 300
temperature = 0.0
```

### Retry and Rate Limiting

```toml
[llm.retry]
max_attempts = 3
initial_delay_ms = 500
max_delay_ms = 30000
multiplier = 2.0
retry_on_rate_limit = true

[llm.throttle]
max_concurrent_requests = 10
requests_per_minute = 500
enabled = true
```

### Fallback

```toml
[llm.fallback]
enabled = true
models = ["gpt-4o-mini", "glm-4-flash"]
on_rate_limit = "retry_then_fallback"
on_timeout = "retry_then_fallback"
on_all_failed = "return_error"
```

### Retrieval Configuration

```toml
[retrieval]
top_k = 3
max_tokens = 1000

[retrieval.search]
top_k = 5
beam_width = 3
max_iterations = 10
min_score = 0.1

[retrieval.sufficiency]
min_tokens = 500
target_tokens = 2000
max_tokens = 4000
confidence_threshold = 0.7

[retrieval.cache]
max_entries = 1000
ttl_secs = 3600
```

### Strategy Configuration

```toml
# Hybrid (BM25 + LLM)
[retrieval.strategy.hybrid]
enabled = true
pre_filter_ratio = 0.3
bm25_weight = 0.4
llm_weight = 0.6

# Cross-document search
[retrieval.strategy.cross_document]
enabled = true
max_documents = 10
max_results_per_doc = 3
merge_strategy = "TopK"
parallel_search = true

# Page range (PDF)
[retrieval.strategy.page_range]
enabled = true
include_boundary_nodes = true
expand_context_pages = 0
```

### Pilot Configuration

```toml
[pilot]
mode = "Balanced"        # "Conservative", "Balanced", "Aggressive"
guide_at_start = true
guide_at_backtrack = true

[pilot.budget]
max_tokens_per_query = 2000
max_tokens_per_call = 500
max_calls_per_query = 5

[pilot.intervention]
fork_threshold = 3
score_gap_threshold = 0.15
low_score_threshold = 0.3
```

### Multi-turn Retrieval

```toml
[retrieval.multiturn]
enabled = true
max_sub_queries = 3
decomposition_model = "gpt-4o-mini"
aggregation_strategy = "merge"
```

### Reference Following

```toml
[retrieval.reference]
enabled = true
max_depth = 3
max_references = 10
follow_pages = true
follow_tables_figures = true
min_confidence = 0.5
```

### Storage Configuration

```toml
[storage]
workspace_dir = "./workspace"
cache_size = 100
atomic_writes = true
file_lock = true
checksum_enabled = true

[storage.compression]
enabled = false
algorithm = "gzip"
level = 6
```

### Metrics

```toml
[metrics]
enabled = true
storage_path = "./workspace/metrics"
retention_days = 30

[metrics.llm]
track_tokens = true
track_latency = true
track_cost = true
cost_per_1k_input_tokens = 0.00015
cost_per_1k_output_tokens = 0.0006

[metrics.pilot]
track_decisions = true
track_accuracy = true
track_feedback = true

[metrics.retrieval]
track_paths = true
track_scores = true
track_iterations = true
track_cache = true
```
