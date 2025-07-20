from __future__ import annotations

from pathlib import Path  # noqa: TC003 -- Typer evaluates this at runtime

import anyio
import typer

from . import pure
from .client import rpc_call
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


def _todo(name: str) -> None:
    """Placeholder for unimplemented commands."""
    typer.echo(f"{name} not implemented", err=True)
    raise typer.Exit(1)


@app.command("project-status")
def project_status() -> None:
    """Check daemon and language server health."""
    anyio.run(rpc_call, "project-status")


@app.command("list-diagnostics")
def list_diagnostics() -> None:  # pragma: no cover - stub
    _todo("list-diagnostics")


@app.command("onboard-project")
def onboard_project() -> None:  # pragma: no cover - stub
    _todo("onboard-project")


@app.command("find-symbol")
def find_symbol() -> None:  # pragma: no cover - stub
    _todo("find-symbol")


@app.command("get-definition")
def get_definition() -> None:  # pragma: no cover - stub
    _todo("get-definition")


@app.command("list-references")
def list_references() -> None:  # pragma: no cover - stub
    _todo("list-references")


@app.command("summarise-symbol")
def summarise_symbol() -> None:  # pragma: no cover - stub
    _todo("summarise-symbol")


@app.command("get-call-graph")
def get_call_graph() -> None:  # pragma: no cover - stub
    _todo("get-call-graph")


@app.command("get-type-hierarchy")
def get_type_hierarchy() -> None:  # pragma: no cover - stub
    _todo("get-type-hierarchy")


@app.command("list-memories")
def list_memories() -> None:  # pragma: no cover - stub
    _todo("list-memories")


@app.command("analyse-impact")
def analyse_impact() -> None:  # pragma: no cover - stub
    _todo("analyse-impact")


@app.command("get-code-actions")
def get_code_actions() -> None:  # pragma: no cover - stub
    _todo("get-code-actions")


@app.command("test")
def cmd_test() -> None:  # pragma: no cover - stub
    _todo("test")


@app.command("build")
def cmd_build() -> None:  # pragma: no cover - stub
    _todo("build")


@app.command("with-transient-edit")
def with_transient_edit() -> None:  # pragma: no cover - stub
    _todo("with-transient-edit")


@app.command("rename-symbol")
def rename_symbol() -> None:  # pragma: no cover - stub
    _todo("rename-symbol")


@app.command("apply-edits")
def apply_edits() -> None:  # pragma: no cover - stub
    _todo("apply-edits")


@app.command("format-code")
def format_code() -> None:  # pragma: no cover - stub
    _todo("format-code")


@app.command("set-active-project")
def set_active_project() -> None:  # pragma: no cover - stub
    _todo("set-active-project")


@app.command("reload-workspace")
def reload_workspace() -> None:  # pragma: no cover - stub
    _todo("reload-workspace")
