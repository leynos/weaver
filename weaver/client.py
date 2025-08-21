from __future__ import annotations

import asyncio
import asyncio.subprocess as aio_subprocess
import logging
import os
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

JSONValue: typ.TypeAlias = (
    bool | int | float | str | list["JSONValue"] | dict[str, "JSONValue"] | None
)
JSONObject: typ.TypeAlias = dict[str, JSONValue]


class DaemonStartError(TimeoutError):
    """Raised when ``weaverd`` fails to become ready."""

    def __init__(self) -> None:
        super().__init__("weaverd failed to start")


def discover_socket() -> Path:
    """Return the daemon socket path."""
    return default_socket_path()


async def spawn_daemon(
    socket_path: Path, *, debug: bool | None = None
) -> aio_subprocess.Process:
    """Spawn ``weaverd`` detached from the controlling terminal."""
    debug_env = os.environ.get("WEAVER_DEBUG", "0")
    debug = bool(int(debug_env)) if debug is None else debug
    return await aio_subprocess.create_subprocess_exec(
        sys.executable,
        "-m",
        "weaverd",
        "--socket-path",
        str(socket_path),
        stdin=aio_subprocess.DEVNULL,
        stdout=None if debug else aio_subprocess.DEVNULL,
        stderr=None if debug else aio_subprocess.DEVNULL,
        start_new_session=True,
    )


async def ensure_daemon_running(socket_path: Path) -> None:
    """Start ``weaverd`` if the socket is unavailable."""
    if await can_connect(socket_path):
        return
    await spawn_daemon(socket_path)
    for _ in range(50):
        if await can_connect(socket_path):
            return
        await anyio.sleep(0.1)
    raise DaemonStartError()


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
    async for line in reader:
        error |= _process_response_line(line, stdout)
    return error


async def rpc_call(
    method: str,
    params: JSONObject | None = None,
    socket_path: Path | None = None,
    stdout: typ.TextIO | None = None,
) -> None:
    """Send an RPC request and stream the response to ``stdout``."""
    path = socket_path or discover_socket()
    stdout = typ.cast(typ.TextIO, sys.stdout if stdout is None else stdout)  # noqa: TC006
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
    return
