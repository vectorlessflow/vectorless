# CLAUDE.md

A hierarchical, reasoning-native document intelligence engine written in Rust.

## Project Structure

- `rust/` - Rust core engine
  - `src/client/` - Client API (EngineBuilder, Engine) - facade layer, no business logic
  - `src/document/` - Document data structures (DocumentTree, NavigationIndex, ReasoningIndex)
  - `src/index/` - Compile pipeline (8-stage, checkpointing, incremental update)
  - `src/retrieval/` - Retrieval dispatch layer (preprocessing, dispatch, postprocessing, cache, streaming)
  - `src/query/` - Query understanding and planning (intent classification, rewrite, decomposition, budget)
  - `src/agent/` - Retrieval execution (SubAgent: doc navigation, Orchestrator: workspace analysis + multi-doc fusion)
  - `src/rerank/` - Result reranking and answer synthesis (dedup, scoring, fusion, synthesis)
  - `src/scoring/` - Scoring and ranking strategies (BM25, relevance scoring, score combination)
  - `src/cache/` - Unified cache abstraction (trait + implementations)
  - `src/llm/` - LLM client (connection pool, memo/caching, throttle/rate-limiting, fallback)
  - `src/storage/` - Persistence (Workspace, LRU cache, backend abstraction file/memory)
  - `src/graph/` - Cross-document relationship graph
  - `src/metrics/` - Metrics collection and reporting
  - `src/events/` - Event system for progress monitoring
  - `src/config/` - Configuration types and validation
  - `src/error.rs` - Unified error types
  - `src/utils/` - Utility functions (token counting, fingerprinting, validation)
  - `examples/` - Rust examples (flow, indexing, pdf, batch, etc.)
- `python/` - Python SDK (PyO3 bindings) + CLI
- `docs/` - Docusaurus documentation site
- `samples/` - Sample files

### Retrieval Call Flow

```
Engine.query()
  → retrieval/dispatcher
    → query/understand() → QueryPlan
    → branch:
        ├── User specified doc_ids → parallel spawn N × SubAgent
        └── Workspace scope → Orchestrator (analyze DocCards → spawn SubAgents → fusion)
    → rerank/ (dedup → score → fusion → synthesis)
```

## Build Commands

```bash
# Rust core
cd rust
cargo build          # Build
cargo test           # Run tests
cargo clippy         # Lint
cargo fmt            # Format code

# Python SDK
cd python
pip install -e .     # Install in editable mode

# Docs site
cd docs
pnpm install         # Install dependencies
pnpm build           # Build static site
```

## Code Conventions

- Follow Rust standard naming (snake_case for functions/variables, PascalCase for types)
- Use `thiserror` for error handling
- Use `tracing` for logging
- Public APIs require documentation comments

---

## ⚠️ Agent Behavior Constraints (IMPORTANT)

### Operations Requiring Confirmation

The following operations **MUST ask for user confirmation** before execution:

#### Irreversible Operations
- `rm`, `rm -rf`, `rmdir` and any file/directory deletion commands
- Destructive git operations: `git push --force`, `git reset --hard`, `git clean -fd`
- Database operations: `DROP TABLE`, `DELETE FROM`
- Clearing or overwriting important configuration files
- Deleting branches (`git branch -D`)

#### Remote/Shared Operations
- `git push` (any form)
- Creating, merging, or closing PRs/Issues
- Sending messages to external services (Slack, Email, etc.)
- Modifying CI/CD configurations
- Publishing packages to crates.io or other registries

#### File Overwrites
- Using `Write` tool to overwrite existing files (unless explicitly requested by user)
- Large-scale batch modifications to multiple files

### Auto-Allowed Operations

The following operations can be executed **without confirmation**:

- Reading files (`Read` tool)
- Searching files (`Glob`, `Grep` tools)
- Editing files (`Edit` tool) - small scope modifications
- Creating new files (not overwriting)
- Running local build/test commands (`cargo build`, `cargo test`, `cargo clippy`)
- Viewing git status (`git status`, `git log`, `git diff`)
- Creating local branches

### Prohibited Operations

The following operations are **ABSOLUTELY FORBIDDEN**:

- Committing files containing sensitive information (`.env`, `credentials.json`, API keys)
- Bypassing pre-commit hooks (`--no-verify`)
- Modifying `.gitignore` to commit ignored sensitive files
- Executing scripts from untrusted sources
- Modifying system-level configurations

### Confirmation Format

When executing dangerous operations, use `AskUserQuestion` tool to explicitly ask:

```
I am about to execute [specific operation], which is an [irreversible/remote/destructive] operation.
Do you want to proceed?
```

### Code Security

- Do not introduce security vulnerabilities (SQL injection, XSS, command injection, etc.)
- Do not hardcode secrets or credentials in code
- Use secure dependency versions
- Validate user input at system boundaries

### Principle of Caution

When uncertain whether an operation is safe, **default to asking user confirmation**.

---

## Common Development Workflow

1. **Adding features**: Implement in appropriate `rust/src/` module, add tests
2. **Fixing bugs**: Add failing test case first, fix and ensure tests pass
3. **Python bindings**: Update `python/src/lib.rs` (PyO3) when Rust APIs change
4. **Committing code**: Use semantic commit messages, format: `type(scope): description`
