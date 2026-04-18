"""CLI application definition and command routing."""

import asyncio
from typing import Optional

import click


@click.group()
@click.version_option(package_name="vectorless")
@click.option("--workspace", "-w", default=".vectorless", help="Workspace directory path.")
@click.pass_context
def app(ctx: click.Context, workspace: str) -> None:
    """Vectorless — reasoning-native document intelligence engine."""
    ctx.ensure_object(dict)
    ctx.obj["workspace"] = workspace


# ── Index commands ──────────────────────────────────────────

@app.command()
@click.option("--workspace", "-w", default=".", help="Directory to initialize.")
def init(workspace: str) -> None:
    """Initialize a .vectorless/ workspace."""
    raise NotImplementedError


@app.command()
@click.argument("path", type=click.Path(exists=True))
@click.option("--recursive", "-r", is_flag=True, help="Index directory recursively.")
@click.option("--format", "fmt", type=click.Choice(["markdown", "pdf"]), help="Force document format.")
@click.option("--force", is_flag=True, help="Force re-index existing documents.")
@click.option("--jobs", "-j", default=1, type=int, help="Parallel indexing jobs.")
@click.option("--verbose", "-v", is_flag=True, help="Show detailed progress.")
def add(
    path: str,
    recursive: bool,
    fmt: Optional[str],
    force: bool,
    jobs: int,
    verbose: bool,
) -> None:
    """Index a document or directory.

    PATH can be a file (.md, .pdf) or a directory.
    """
    raise NotImplementedError


@app.command("list")
@click.option("--format", "fmt", type=click.Choice(["table", "json"]), default="table")
def list_cmd(fmt: str) -> None:
    """List all indexed documents."""
    raise NotImplementedError


@app.command()
@click.argument("doc_id")
def info(doc_id: str) -> None:
    """Show details of an indexed document."""
    raise NotImplementedError


@app.command()
@click.argument("doc_id")
@click.confirmation_option(prompt="Remove this document index?")
def remove(doc_id: str) -> None:
    """Remove a document from the index."""
    raise NotImplementedError


# ── Query commands ──────────────────────────────────────────

@app.command()
@click.argument("question")
@click.option("--doc", "-d", multiple=True, help="Limit query to specific document IDs.")
@click.option("--workspace-scope", is_flag=True, help="Query across all documents.")
@click.option("--format", "fmt", type=click.Choice(["text", "json"]), default="text")
@click.option("--verbose", "-v", is_flag=True, help="Show Agent navigation steps.")
@click.option("--max-tokens", type=int, help="Max result tokens.")
def query(
    question: str,
    doc: tuple[str, ...],
    workspace_scope: bool,
    fmt: str,
    verbose: bool,
    max_tokens: Optional[int],
) -> None:
    """Query indexed documents.

    QUESTION is the natural-language question to ask.
    """
    raise NotImplementedError


@app.command()
@click.option("--doc", "-d", help="Limit to a specific document ID.")
@click.option("--verbose", "-v", is_flag=True, help="Show Agent navigation steps.")
def ask(doc: Optional[str], verbose: bool) -> None:
    """Interactive query REPL.

    Start a multi-turn conversation with your documents.
    """
    raise NotImplementedError


# ── Debug / tool commands ───────────────────────────────────

@app.command()
@click.argument("doc_id")
@click.option("--depth", "-d", type=int, help="Max depth to display.")
@click.option("--show-summary", is_flag=True, help="Show node summaries.")
@click.option("--show-keywords", is_flag=True, help="Show routing keywords.")
def tree(doc_id: str, depth: Optional[int], show_summary: bool, show_keywords: bool) -> None:
    """Visualize document tree structure."""
    raise NotImplementedError


@app.command()
def stats() -> None:
    """Show workspace statistics."""
    raise NotImplementedError


@app.command("config")
@click.argument("key", required=False)
@click.argument("value", required=False)
@click.option("--init", "init_config", is_flag=True, help="Re-initialize default config.")
def config_cmd(key: Optional[str], value: Optional[str], init_config: bool) -> None:
    """View or modify configuration.

    \b
    vectorless-cli config                    Show all config
    vectorless-cli config llm.model          Show specific key
    vectorless-cli config llm.model gpt-4o   Set a value
    vectorless-cli config --init             Reset to defaults
    """
    raise NotImplementedError
