from __future__ import annotations

import asyncio
import logging
import os
import subprocess
import sys
import typing as typ
from pathlib import Path  # noqa: TC003

import anyio
import msgspec as ms
import msgspec.json as msjson
import typer

from weaverd.server import default_socket_path

from .errors import is_dependency_error
from .sockets import can_connect

logger = logging.getLogger(__name__)


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


def _process_response_line(data: bytes, stdout: typ.TextIO) -> bool:
    """Write ``data`` to ``stdout`` and detect dependency errors.

    Returns:
        bool: ``True`` if a dependency error was detected, ``False`` otherwise.
    """

    text = data.decode(
        encoding=getattr(stdout, "encoding", "utf-8") or "utf-8",
        errors="replace",
    )
    stdout.write(text)
    stdout.flush()

    try:
        record = msjson.decode(data.rstrip())
    except ms.DecodeError as exc:
        logger.warning("Failed to decode response line: %r: %s", data, exc)
        return False
    return bool(isinstance(record, dict) and is_dependency_error(record))


async def _stream_response(reader: asyncio.StreamReader, stdout: typ.TextIO) -> bool:
    """Stream lines from ``reader`` to ``stdout`` and flag dependency errors."""

    error = False
    while data := await reader.readline():
        if _process_response_line(data, stdout):
            error = True
    return error


async def rpc_call(
    method: str,
    params: dict[str, typ.Any] | None = None,
    socket_path: Path | None = None,
    stdout: typ.TextIO | None = None,
) -> None:
    """Send an RPC request and stream the response to ``stdout``."""
    path = socket_path or discover_socket()
    stdout = typ.cast("typ.TextIO", sys.stdout if stdout is None else stdout)
    try:
        await ensure_daemon_running(path)
    except Exception as exc:
        print(f"Error: Could not ensure daemon is running: {exc}", file=sys.stderr)
        raise typer.Exit(1) from exc

    reader, writer = await asyncio.open_unix_connection(str(path))
    error = False
    try:
        writer.write(msjson.encode({"method": method, "params": params or {}}) + b"\n")
        await writer.drain()
        writer.write_eof()
        error = await _stream_response(reader, stdout)
    finally:
        writer.close()
        await writer.wait_closed()
    if error:
        raise typer.Exit(1)
