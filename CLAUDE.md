# CLAUDE.md

Vectorless is a Document Understanding Engine for AI. See `README.md` for the project's public positioning.

## Positioning: a standard + a reference implementation

Vectorless is split into two layers that are **independently consumable**:

1. **The Rust side defines two standards.**
   - **The IR** (`vectorless-document::Document`) — a single, versioned, serializable artifact that fully represents an understood document. Produced by the compile pipeline; can be persisted, transmitted, and re-loaded.
   - **The navigation primitives** (`vectorless-primitives::DocumentNavigator`) — a fixed set of shell-style operations (`ls`, `cd`, `cat`, `grep`, `find`, `head`, `wc`, `pwd`, `back`, plus inspection and reasoning-index queries) that any agent uses to read an IR. Exposed to Python via PyO3.

   These two together are **the standard**. Anything that produces a valid `Document` and consumes it via `DocumentNavigator` is a conforming participant.

2. **The Python side is one reference agent implementation.** `vectorless/ask/` is a multi-agent system (Orchestrator + Workers + reasoning + verify) that ships in the box. It is the default `engine.ask()` — but it is not load-bearing for the standard. Developers can:
   - Replace `ask/` entirely with their own Python agent built on the same `Document` / `DocumentNavigator` primitives.
   - Skip Python and consume the IR + primitives directly from Rust.
   - Produce IRs from a non-Rust source (the IR is serializable JSON) and feed them into any compatible navigator/agent.

The reason this matters: the value of Vectorless is the **IR + primitives contract**, not the bundled reasoning loop. The reference implementation is a courtesy, not a lock-in.

## Project Structure

```
crates/                       Rust — the standard
├── vectorless-error/         Result / Error
├── vectorless-document/      ★ IR types: Document, DocumentTree, NavigationIndex, ReasoningIndex,
│                              QueryRoutingTable, ChainIndex, ContentOverlapMap, EvidenceScoreMap
├── vectorless-primitives/    ★ DocumentNavigator (the navigation API)
├── vectorless-config/        Configuration hub
├── vectorless-utils/         Fingerprinting, token counting, keyword extraction
├── vectorless-graph/         Cross-document relationship graph
├── vectorless-events/        Progress event bus
├── vectorless-metrics/       Metrics collection
├── vectorless-llm/           LLM client (pool, throttle, retry)
├── vectorless-storage/       Workspace persistence (file backend, LRU cache)
├── vectorless-compiler/      Compile pipeline (frontend → transform → analysis → backend)
├── vectorless-engine/        Facade: Engine, EngineBuilder, compile() / forget() / load_document()
└── vectorless-py/            PyO3 bindings → produces the `_vectorless` native module

vectorless/                   Python — the reference multi-agent implementation
├── ask/                      Orchestrator + Worker + reasoning + verify (the default agent)
├── rerank/                   Dedup + quality scoring + synthesis
├── _internal/                Wrappers around the PyO3 native module
├── engine.py                 User-facing Engine (delegates compile to Rust, ask to ask/)
└── cli/                      CLI entrypoint

examples/                     Python examples
docs/                         Docusaurus documentation site
```

The two crates marked ★ are the **standard surface**. Everything else in `crates/` is implementation detail that produces and serves that standard.

### Dependency Layers (Rust)

```
Layer 0:  error · document                              (no workspace deps)
Layer 1:  utils · graph · events                        (depends on Layer 0)
Layer 2:  config · metrics                              (depends on Layer 0–1)
Layer 3:  llm · storage · primitives                    (depends on Layer 0–2)
Layer 4:  compiler                                      (depends on Layer 0–3)
Layer 5:  engine                                        (depends on Layer 0–4)
Layer 6:  vectorless-py (PyO3 bindings)                 (engine + document + primitives)
```

`vectorless-document` and `vectorless-primitives` have no dependency on `compiler`, `llm`, `storage`, or `engine` — by design, so the standard can be consumed without dragging in the pipeline.

### Compile Pipeline (the IR producer)

The compiler runs documents through four stage groups (`crates/vectorless-compiler/src/passes/`):

```
frontend  (parse, build)              raw bytes → DocumentTree
transform (split, enrich)             chunking + section enrichment
analysis  (validate, enhance)         structure checks + augmentation
backend   (route, concept, navigation,
           chain, overlap, reasoning,
           score, optimize, verify)   retrieval-acceleration artifacts
```

Every backend pass attaches an optional acceleration field to the `Document`. The IR is valid without any of them (an agent can navigate using only `tree` + `nav_index`); the acceleration fields exist so well-equipped agents can short-circuit reasoning.

### Compilation Isolation

改一个模块只重编译该 crate + 上游 facade：

- 改 `llm` → llm, compiler, engine, py 重编译；storage/graph 不动
- 改 `compiler` → compiler, engine, py 重编译；llm/storage 不动
- 改 `document` 或 `primitives` → 全部重编译（标准变更，预期行为）
- 改 Python `ask/` / `rerank/` → 不触发 Rust 重编译

### How third parties consume the standard

Anyone building a custom agent works against two types and two crates only:

```rust
use vectorless_document::Document;             // The IR
use vectorless_primitives::DocumentNavigator;  // The primitive API

let doc: Document = load_ir_from_disk()?;       // produced by our compiler, or yours
let mut nav = DocumentNavigator::new(doc);
let children = nav.ls().await;
nav.cd("n3").await?;
let body = nav.cat(None).await?;
```

The Python reference agent (`vectorless/ask/`) is just one consumer of this same surface.

## Build Commands

```bash
# Rust workspace
cargo build          # Build all crates
cargo test           # Run workspace tests
cargo clippy         # Lint
cargo fmt            # Format

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

1. **Touching the standard (`document` or `primitives`):** this is a public-contract change. Bump `CURRENT_SCHEMA_VERSION` if the IR's serialized shape changes; document the new field; add tests.
2. **Adding a compile-pipeline pass:** implement under `crates/vectorless-compiler/src/passes/` (frontend/transform/analysis/backend), wire into the pipeline, attach output to `Document` as an `Option<...>` so old IRs still load.
3. **Adding/modifying reasoning behavior:** implement under `vectorless/ask/` (Python). This is reference-implementation work, not standard work — it should not require Rust changes.
4. **Fixing bugs:** write a failing test first, then fix.
5. **Adding crates:** new modules get their own crate under `crates/`, registered in workspace `Cargo.toml`.
6. **PyO3 bindings:** update `crates/vectorless-py/src/lib.rs` when Rust types cross the FFI boundary; corresponding Python wrappers in `vectorless/_internal/`.
7. **Committing:** Conventional Commits — `feat(compiler): ...`, `fix(ask): ...`, `refactor(primitives): ...`.
