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


def _run_rpc(method: str, params: dict | None = None) -> None:
    """Execute an RPC request and handle failures uniformly."""
    try:
        anyio.run(rpc_call, method, params)
    except Exception as exc:
        # We surface the raw error to aid debugging while keeping exit codes
        # consistent for callers that rely on them.
        typer.echo(f"Error: {exc}", err=True)
        raise typer.Exit(1) from exc


@app.command("project-status")
def project_status() -> None:
    """Check daemon and language server health."""
    _run_rpc("project-status")


@app.command("list-diagnostics")
def list_diagnostics(
    severity: str | None = typer.Option(
        None,
        "--severity",
        "-s",
        help="Filter diagnostics by severity",
    ),
    files: list[Path] = typer.Argument(None, metavar="[FILES...]"),  # noqa: B008
) -> None:
    """Stream diagnostics for the workspace."""
    params: dict[str, typ.Any] = {}
    if severity:
        params["severity"] = severity
    if files:
        params["files"] = [str(p) for p in files]
    _run_rpc("list-diagnostics", params or None)


@app.command("onboard-project")
def onboard_project() -> None:
    """Perform first-run project analysis."""
    _run_rpc("onboard-project")


@app.command("get-definition")
def get_definition(file: Path, line: int, char: int) -> None:
    """Locate the symbol definition at the given position."""

    params = {"file": str(file), "line": line, "char": char}
    _run_rpc("get-definition", params)


STUBS = [
    "find-symbol",
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
