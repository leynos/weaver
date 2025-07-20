from __future__ import annotations

import subprocess
import sys
import typing as t

import anyio

from weaverd.server import default_socket_path

from .sockets import can_connect

if t.TYPE_CHECKING:
    from pathlib import Path


async def ensure_running(path: Path | None = None, timeout: float = 5.0) -> Path:
    """Ensure the daemon is running and return its socket path."""
    sock = path or default_socket_path()
    if await can_connect(sock):
        return sock
    subprocess.Popen(  # noqa: S603
        [sys.executable, "-m", "weaverd", "--socket-path", str(sock)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    for _ in range(int(timeout / 0.1)):
        if await can_connect(sock):
            return sock
        await anyio.sleep(0.1)
    raise RuntimeError("weaverd failed to start")
