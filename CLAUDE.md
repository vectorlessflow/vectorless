# CLAUDE.md

Vectorless is a Document Understanding Engine for AI written in Rust.

## Principles

- **Reason, don't vector.** Retrieval is a reasoning act, not a similarity computation.
- **Model fails, we fail.** No heuristic fallbacks, no silent degradation.
- **No thought, no answer.** Only reasoned output counts as an answer.

## Project Structure

Cargo workspace with 17 fine-grained Rust crates + pure Python SDK:

```
crates/
├── vectorless-error/       # Error types (Result, Error enum)
├── vectorless-document/    # Document types (Document, Tree, NavigationIndex, ReasoningIndex)
├── vectorless-config/      # Configuration hub (aggregates all config types)
├── vectorless-utils/       # Utilities (fingerprinting, token counting, validation)
├── vectorless-scoring/     # Scoring (BM25, keyword extraction)
├── vectorless-graph/       # Cross-document relationship graph
├── vectorless-events/      # Event system for progress monitoring
├── vectorless-metrics/     # Metrics collection and reporting
├── vectorless-llm/         # LLM client (pool, memo/cache, throttle, fallback)
├── vectorless-storage/     # Persistence (Workspace, LRU cache, file/memory backends)
├── vectorless-query/       # Query understanding (intent classification, rewrite)
├── vectorless-compiler/    # Compile pipeline (15-pass, checkpointing, incremental update)
├── vectorless-agent/       # Retrieval execution (Worker navigation + Orchestrator fusion)
├── vectorless-retrieval/   # Retrieval dispatch layer (dispatcher, cache, streaming)
├── vectorless-rerank/      # Result reranking (dedup, BM25 scoring, fusion)
├── vectorless-engine/      # Facade (Engine, EngineBuilder) — re-exports public API
└── vectorless-py/          # PyO3 bindings (compiled into Python native module)
```

- `vectorless/` - Pure Python SDK (high-level wrappers, CLI, config loading, integrations)
- `examples/` - Python examples (primary, for Python ecosystem)
- `docs/` - Docusaurus documentation site

### Dependency Layers

```
Layer 0:  error · document · utils · scoring          (no workspace deps)
Layer 1:  graph · events · config · metrics            (depends on Layer 0)
Layer 2:  llm · storage                                 (depends on Layer 0–1)
Layer 3:  query                                         (depends on Layer 0–2)
Layer 4:  compiler · agent                               (depends on Layer 0–3)
Layer 5:  retrieval · rerank                            (depends on Layer 0–4)
Layer 6:  engine (facade) · vectorless-py (bindings)    (depends on all)
```

### Compilation Isolation

改一个模块只重编译该 crate + 上游 facade：
- 改 `agent` → agent, retrieval, rerank, engine, py 重编译；index/llm/storage 不动
- 改 `llm` → llm 及其上层重编译；index/agent/stage 不重编译
- 改 `document` → 全部重编译（核心类型，预期行为）

### Retrieval Call Flow

```
Engine.ask()
  → retrieval/dispatcher
    → query/understand() → QueryPlan (LLM intent + concepts + strategy)
    → Orchestrator (always, single or multi-doc)
      → analyze(QueryPlan) → dispatch plan
      → supervisor loop:
          dispatch Workers → evaluate() →
          if insufficient → replan() → loop
      → rerank/ (dedup → BM25 score → synthesis/fusion)
```

## Build Commands

```bash
# Build (workspace)
cargo build          # Build all crates
cargo test           # Run tests (488 tests across all crates)
cargo clippy         # Lint
cargo fmt            # Format code

# Build specific crate (fast — only that crate + dependents)
cargo build -p vectorless-agent

# Python SDK
pip install -e .     # Install in editable mode (from project root, uses maturin)

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

1. **Adding features**: Implement in the appropriate `crates/vectorless-*/` crate, add tests
2. **Fixing bugs**: Add failing test case first, fix and ensure tests pass
3. **Adding crates**: New modules get their own crate under `crates/`, add to workspace Cargo.toml
4. **Python bindings**: Update `crates/vectorless-py/src/lib.rs` (PyO3) when Rust APIs change
5. **Python SDK**: Update `vectorless/` when API surface changes
6. **Committing code**: Use semantic commit messages, format: `type(scope): description`
