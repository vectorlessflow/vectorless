# CLAUDE.md

Vectorless is a Document Understanding Engine for AI. Compile pipeline is written in Rust; the reasoning/retrieval (ask) layer is written in Python on top of LLM tool-use. See `README.md` for the project's public positioning.

## Project Structure

Cargo workspace with **13 Rust crates** (compile pipeline + bindings) plus a **Python package** (`vectorless/`) that owns the ask/reasoning loop.

### Rust crates (`crates/`)

```
crates/
├── vectorless-error/        # Error types (Result, Error enum)
├── vectorless-document/     # Document types (Document, Tree, NavigationIndex)
├── vectorless-config/       # Configuration hub (aggregates all config types)
├── vectorless-utils/        # Utilities (fingerprinting, token counting, keyword extraction)
├── vectorless-graph/        # Cross-document relationship graph
├── vectorless-events/       # Event system for progress monitoring
├── vectorless-metrics/      # Metrics collection and reporting
├── vectorless-llm/          # LLM client (pool, memo/cache, throttle)
├── vectorless-storage/      # Persistence (Workspace, LRU cache, file backend)
├── vectorless-compiler/     # Compile pipeline (frontend → transform → analysis → backend)
├── vectorless-primitives/   # Document navigation primitives (DocumentNavigator)
├── vectorless-engine/       # Facade (Engine, EngineBuilder) — re-exports public API
└── vectorless-py/           # PyO3 bindings (compiled into Python native module)
```

### Python package (`vectorless/`)

```
vectorless/
├── ask/                     # Reasoning loop (Orchestrator + Workers + supervisor)
│   ├── orchestrator.py      # Top-level coordinator
│   ├── worker/              # Navigation Worker (ls/cd/cat/grep/find/head/wc/chain)
│   ├── dispatcher.py        # Worker dispatch
│   ├── plan.py              # Replanning
│   ├── understand.py        # Query understanding → QueryPlan
│   ├── reasoning/           # Reasoning chain analyzer
│   ├── evaluate.py          # Evidence sufficiency evaluation
│   ├── verify/              # Answer verifier
│   ├── blackboard.py        # Shared evidence state
│   └── prompts.py
├── rerank/                  # Dedup + quality scoring + synthesis
├── _internal/               # Wrappers around the PyO3 native module
├── engine.py                # User-facing Engine class
└── cli/                     # CLI entrypoint
```

- `examples/` — Python examples (primary, for Python ecosystem)
- `docs/` — Docusaurus documentation site

### Dependency Layers (Rust)

```
Layer 0:  error · document                              (no workspace deps)
Layer 1:  utils · graph · events                        (depends on Layer 0)
Layer 2:  config · metrics                              (depends on Layer 0–1)
Layer 3:  llm · storage · primitives                    (depends on Layer 0–2)
Layer 4:  compiler                                      (depends on Layer 0–3)
Layer 5:  engine                                        (depends on Layer 0–4)
Layer 6:  vectorless-py (PyO3 bindings)                 (depends on engine + primitives)
```

### Compile Pipeline

The compiler runs documents through four stage groups (`crates/vectorless-compiler/src/passes/`):

```
frontend  (parse, build)              ← raw bytes → DocumentTree
transform (split, enrich)             ← chunking + section enrichment
analysis  (validate, enhance)         ← structure checks + augmentation
backend   (route, concept, navigation,
           chain, overlap, reasoning,
           score, optimize, verify)   ← retrieval-acceleration artifacts
```

### Compilation Isolation

改一个模块只重编译该 crate + 上游 facade：
- 改 `llm` → llm, compiler, engine, py 重编译；storage/graph 不动
- 改 `compiler` → compiler, engine, py 重编译；llm/storage 不动
- 改 `document` → 全部重编译（核心类型，预期行为）
- 改 Python `ask/`、`rerank/` → 不触发 Rust 重编译

### Ask Call Flow

```
engine.ask()  [Python]
  → ask/understand()        → QueryPlan (intent + concepts + strategy, LLM-driven)
  → ask/orchestrator
      ├── analyze(QueryPlan)            → dispatch plan
      └── supervisor loop:
          dispatch Workers (Rust DocumentNavigator via PyO3)
            → execute nav commands (ls/cd/cat/grep/find/head/wc/chain)
          → evaluate(blackboard)        → sufficiency check
          → if insufficient: plan.replan() → loop
      → rerank/ (dedup → quality score → synthesize)
  → verify/                  → final answer check
```

The Rust side exposes `DocumentNavigator` (`vectorless-primitives`) and compiled artifacts; the Python orchestrator drives the LLM reasoning loop and calls back into Rust for fast document operations.

## Build Commands

```bash
# Rust workspace
cargo build          # Build all crates
cargo test           # Run workspace tests
cargo clippy         # Lint
cargo fmt            # Format code

# Build a single crate (fast — only that crate + dependents)
cargo build -p vectorless-compiler

# Python SDK (uses maturin under the hood)
pip install -e .     # Editable install from project root

# Docs site
cd docs
pnpm install
pnpm build
```

## Code Conventions

- Rust: snake_case for functions/variables, PascalCase for types; `thiserror` for errors; `tracing` for logging; doc comments on public APIs.
- Python: type-annotated; `pydantic` for models; `litellm` + `instructor` for LLM calls.
- Commit messages: `type(scope): description` (Conventional Commits).

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

1. **Adding compile-pipeline features**: implement under `crates/vectorless-compiler/src/passes/` (frontend/transform/analysis/backend), add tests in the same module.
2. **Adding reasoning/ask features**: implement under `vectorless/ask/` (Python), add prompts in `prompts.py`.
3. **Fixing bugs**: write a failing test first, then fix.
4. **Adding crates**: new modules get their own crate under `crates/`, registered in workspace `Cargo.toml`.
5. **Python bindings**: update `crates/vectorless-py/src/lib.rs` (PyO3) when Rust APIs cross the FFI boundary; corresponding wrappers go in `vectorless/_internal/`.
6. **Python SDK surface**: update `vectorless/engine.py` and related modules when the public API changes.
7. **Committing**: use Conventional Commits — `feat(compiler): ...`, `fix(ask): ...`, `refactor(engine): ...`.
