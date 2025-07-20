from __future__ import annotations

import asyncio
import io  # noqa: TC003
import os
import subprocess
import sys
import typing as t
from pathlib import Path  # noqa: TC003

import anyio
import typer
from msgspec import json

from weaverd.server import default_socket_path

from .sockets import can_connect


def discover_socket() -> Path:
    """Return the daemon socket path."""
    return default_socket_path()


def spawn_daemon(
    socket_path: Path, *, debug: bool | None = None
) -> subprocess.Popen[bytes]:
    """Spawn ``weaverd`` detached from the controlling terminal."""
    debug_env = os.environ.get("WEAVER_DEBUG", "0")
    debug = bool(int(debug_env)) if debug is None else debug
    return subprocess.Popen(  # noqa: S603 -- trusted internal command
        [sys.executable, "-m", "weaverd", "--socket-path", str(socket_path)],
        stdin=subprocess.DEVNULL,
        stdout=None if debug else subprocess.DEVNULL,
        stderr=None if debug else subprocess.DEVNULL,
        start_new_session=True,
    )


async def ensure_daemon_running(socket_path: Path) -> None:
    """Start ``weaverd`` if the socket is unavailable."""
    if await can_connect(socket_path):
        return
    spawn_daemon(socket_path)
    for _ in range(50):
        if await can_connect(socket_path):
            return
        await anyio.sleep(0.1)
    raise RuntimeError("weaverd failed to start")


async def rpc_call(
    method: str,
    params: dict[str, t.Any] | None = None,
    socket_path: Path | None = None,
    stdout: t.TextIO | None = None,
) -> None:
    """Send an RPC request and stream the response to ``stdout``."""
    path = socket_path or discover_socket()
    stdout = t.cast("t.TextIO", sys.stdout if stdout is None else stdout)
    try:
        await ensure_daemon_running(path)
    except Exception as exc:
        print(f"Error: Could not ensure daemon is running: {exc}", file=sys.stderr)
        raise typer.Exit(1) from exc

    reader, writer = await asyncio.open_unix_connection(str(path))
    try:
        writer.write(json.encode({"method": method, "params": params or {}}) + b"\n")
        await writer.drain()
        writer.write_eof()
        while data := await reader.readline():
            buf: io.BufferedWriter | None = getattr(stdout, "buffer", None)
            if buf is not None:
                buf.write(data)
            else:
                stdout.write(data.decode())
            stdout.flush()
    finally:
        writer.close()
        await writer.wait_closed()
