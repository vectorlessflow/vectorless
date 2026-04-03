# Contributing to Vectorless

Thank you for considering contributing to Vectorless!

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How to Contribute](#how-to-contribute)
- [Development Setup](#development-setup)
- [Code Standards](#code-standards)
- [Commit Guidelines](#commit-guidelines)
- [Pull Request Process](#pull-request-process)

## Code of Conduct

This project adheres to the [Contributor Covenant](CODE_OF_CONDUCT.md) code of conduct. By participating, you are expected to uphold this code.

## How to Contribute

### Reporting Bugs

If you find a bug, please [create an issue](https://github.com/vectorlessflow/vectorless/issues/new) including:

- A clear description of the problem
- Steps to reproduce
- Expected behavior
- Actual behavior
- Rust version and operating system

### Suggesting Features

Feature suggestions are welcome! Please create an issue describing:

- The feature you'd like to see
- Use cases
- Possible implementation approach (optional)

### Submitting Code

See [Pull Request Process](#pull-request-process) below.

## Development Setup

### Requirements

- Rust 1.85+
- cargo

### Building

```bash
# Clone the repository
git clone https://github.com/vectorlessflow/vectorless.git
cd vectorless

# Build
cargo build

# Run tests
cargo test

# Run examples
cargo run --example basic
```

### Project Structure

```
vectorless/
├── src/
│   ├── client/      # High-level API (Engine, EngineBuilder)
│   ├── domain/      # Core types (DocumentTree, TreeNode)
│   ├── parser/      # Document parsers (Markdown, PDF, DOCX)
│   ├── index/       # Index Pipeline
│   ├── retrieval/   # Retrieval Pipeline
│   ├── llm/         # LLM client
│   ├── storage/     # Persistence
│   └── config/      # Configuration
├── examples/        # Example code
├── tests/           # Integration tests
└── docs/            # Documentation
```

## Code Standards

### Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy -- -D warnings
```

All code must pass clippy with no warnings.

### Documentation

Public APIs must have documentation comments:

```rust
/// Brief description.
///
/// # Examples
///
/// ```
/// use vectorless::Engine;
/// ```
pub fn my_function() {}
```

### Testing

- New features must include unit tests
- Bug fixes must include regression tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name
```

## Commit Guidelines

Use [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation changes |
| `style` | Code style (formatting, no code change) |
| `refactor` | Code refactoring |
| `perf` | Performance improvement |
| `test` | Adding/updating tests |
| `chore` | Build/tooling changes |

### Examples

```
feat(parser): add HTML document support

fix(retrieval): fix backtracking logic in judge stage

docs(readme): update installation instructions
```

## Pull Request Process

1. **Fork** the repository
2. **Create a branch**: `git checkout -b feat/my-feature`
3. **Make changes**: Follow [code standards](#code-standards)
4. **Run checks**:
   ```bash
   cargo fmt -- --check
   cargo clippy -- -D warnings
   cargo test
   ```
5. **Push**: `git push origin feat/my-feature`
6. **Create Pull Request**

### PR Checklist

- [ ] Code passes `cargo fmt -- --check`
- [ ] Code passes `cargo clippy -- -D warnings`
- [ ] All tests pass `cargo test`
- [ ] New features have documentation
- [ ] New features have tests
- [ ] Commit messages follow guidelines

### Review Process

1. Automated CI checks must pass
2. At least one maintainer review
3. Merge after all issues resolved

## Questions?

If you have questions, please [create an issue](https://github.com/vectorlessflow/vectorless/issues) or email beautifularea@gmail.com.

---

Thank you for contributing!
