from __future__ import annotations

from pathlib import Path  # noqa: TC003 -- Typer evaluates this at runtime

import anyio
import typer

from weaverd.server import default_socket_path

from . import pure
from .daemon import ensure_running
from .rpc_client import call_rpc
from .sockets import can_connect

app = typer.Typer(name="weaver")


@app.command()
def hello() -> None:
    """Print a friendly greeting."""
    typer.echo(pure.hello())


@app.command()
def check_socket(path: Path) -> None:
    """Check if a UNIX socket is accepting connections."""
    connected = anyio.run(can_connect, path)
    if connected:
        typer.echo(f"socket available: {path}")
    else:
        typer.echo(f"socket unavailable: {path}")


@app.command("project-status")
def project_status() -> None:
    """Health check of daemon and language servers."""
    anyio.run(_project_status)


async def _project_status() -> None:
    sock = await ensure_running(default_socket_path())
    await call_rpc(sock, "project-status")


def _stub(command: str) -> None:
    typer.echo(f"The '{command}' command is not yet implemented.")


# --- Command Stubs ---------------------------------------------------------


@app.command("list-diagnostics")
def list_diagnostics() -> None:  # pragma: no cover - stub
    """Stream diagnostics for workspace."""
    _stub("list-diagnostics")


@app.command("onboard-project")
def onboard_project() -> None:  # pragma: no cover - stub
    """Perform first-run analysis."""
    _stub("onboard-project")


@app.command("find-symbol")
def find_symbol() -> None:  # pragma: no cover - stub
    """Search workspace symbols."""
    _stub("find-symbol")


@app.command("get-definition")
def get_definition() -> None:  # pragma: no cover - stub
    """Locate the definitive declaration."""
    _stub("get-definition")


@app.command("list-references")
def list_references() -> None:  # pragma: no cover - stub
    """List all references of a symbol."""
    _stub("list-references")


@app.command("summarise-symbol")
def summarise_symbol() -> None:  # pragma: no cover - stub
    """Aggregate hover, docstring, and type info."""
    _stub("summarise-symbol")


@app.command("get-call-graph")
def get_call_graph() -> None:  # pragma: no cover - stub
    """Retrieve the call graph."""
    _stub("get-call-graph")


@app.command("get-type-hierarchy")
def get_type_hierarchy() -> None:  # pragma: no cover - stub
    """Retrieve the type hierarchy."""
    _stub("get-type-hierarchy")


@app.command("list-memories")
def list_memories() -> None:  # pragma: no cover - stub
    """List stored memory snippets."""
    _stub("list-memories")


@app.command("analyse-impact")
def analyse_impact() -> None:  # pragma: no cover - stub
    """Analyse potential impact of an edit."""
    _stub("analyse-impact")


@app.command("get-code-actions")
def get_code_actions() -> None:  # pragma: no cover - stub
    """Retrieve available code actions."""
    _stub("get-code-actions")


@app.command()
def test() -> None:  # pragma: no cover - stub
    """Run project tests."""
    _stub("test")


@app.command()
def build() -> None:  # pragma: no cover - stub
    """Run project build."""
    _stub("build")


@app.command("with-transient-edit")
def with_transient_edit() -> None:  # pragma: no cover - stub
    """Run command with in-memory overlay."""
    _stub("with-transient-edit")


@app.command("rename-symbol")
def rename_symbol() -> None:  # pragma: no cover - stub
    """Rename a symbol across the project."""
    _stub("rename-symbol")


@app.command("apply-edits")
def apply_edits() -> None:  # pragma: no cover - stub
    """Apply a stream of code edits."""
    _stub("apply-edits")


@app.command("format-code")
def format_code() -> None:  # pragma: no cover - stub
    """Format source files."""
    _stub("format-code")


@app.command("set-active-project")
def set_active_project() -> None:  # pragma: no cover - stub
    """Switch the active project."""
    _stub("set-active-project")


@app.command("reload-workspace")
def reload_workspace() -> None:  # pragma: no cover - stub
    """Force reindex of the active project."""
    _stub("reload-workspace")
