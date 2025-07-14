from __future__ import annotations

from pathlib import Path  # noqa: TC003 -- Typer evaluates this at runtime

import anyio
import typer

from . import pure
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
