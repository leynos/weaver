from __future__ import annotations

import asyncio
import os
import tempfile
from pathlib import Path

from weaver_schemas.status import ProjectStatus

from .rpc import RPCDispatcher


def default_socket_path() -> Path:
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR", tempfile.gettempdir())
    user = os.environ.get("USER", "unknown")
    return Path(runtime_dir) / f"weaverd-{user}.sock"


async def handle_client(
    reader: asyncio.StreamReader,
    writer: asyncio.StreamWriter,
    dispatcher: RPCDispatcher,
) -> None:
    try:
        data = await reader.readline()
        if not data:
            return
        response = await dispatcher.handle(data.rstrip())
        writer.write(response + b"\n")
        await writer.drain()
    finally:
        writer.close()
        await writer.wait_closed()


async def start_server(path: Path, dispatcher: RPCDispatcher) -> asyncio.AbstractServer:
    if path.exists():
        path.unlink()
    return await asyncio.start_unix_server(
        lambda r, w: handle_client(r, w, dispatcher), path=str(path)
    )


async def main(socket_path: Path | None = None) -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("project-status")
    async def project_status() -> ProjectStatus:
        return ProjectStatus(message="ok")

    path = socket_path or default_socket_path()
    server = await start_server(path, dispatcher)
    async with server:
        await server.serve_forever()
