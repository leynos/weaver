from __future__ import annotations

import typing as typ
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


def _make_stub(name: str) -> typ.Callable[[], None]:
    def command() -> None:  # pragma: no cover - stub
        _todo(name)

    return command


@app.command("project-status")
def project_status() -> None:
    """Check daemon and language server health."""
    try:
        anyio.run(rpc_call, "project-status")
    except Exception as exc:
        typer.echo(f"Error: {exc}", err=True)
        raise typer.Exit(1) from exc


@app.command("list-diagnostics")
def list_diagnostics() -> None:
    """Stream diagnostics for the workspace."""
    try:
        anyio.run(rpc_call, "list-diagnostics")
    except Exception as exc:
        typer.echo(f"Error: {exc}", err=True)
        raise typer.Exit(1) from exc


@app.command("onboard-project")
def onboard_project() -> None:
    """Perform first-run project analysis."""
    try:
        anyio.run(rpc_call, "onboard-project")
    except Exception as exc:
        typer.echo(f"Error: {exc}", err=True)
        raise typer.Exit(1) from exc


STUBS = [
    "find-symbol",
    "get-definition",
    "list-references",
    "summarise-symbol",
    "get-call-graph",
    "get-type-hierarchy",
    "list-memories",
    "analyse-impact",
    "get-code-actions",
    "test",
    "build",
    "with-transient-edit",
    "rename-symbol",
    "apply-edits",
    "format-code",
    "set-active-project",
    "reload-workspace",
]

for name in STUBS:
    app.command(name)(_make_stub(name))
