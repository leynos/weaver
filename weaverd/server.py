from __future__ import annotations

import asyncio
import getpass
import os
import tempfile
from pathlib import Path

import msgspec.json as msjson

from weaver_schemas.error import SchemaError
from weaver_schemas.status import ProjectStatus

from .rpc import RPCDispatcher


def default_socket_path() -> Path:
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR", tempfile.gettempdir())
    user = getpass.getuser()
    return Path(runtime_dir) / f"weaverd-{user}.sock"


async def handle_client(
    reader: asyncio.StreamReader,
    writer: asyncio.StreamWriter,
    dispatcher: RPCDispatcher,
) -> None:
    try:
        while data := await reader.readline():
            try:
                response = await dispatcher.handle(data.rstrip())
            except Exception as exc:  # pragma: no cover - fallback
                if isinstance(exc, asyncio.CancelledError | KeyboardInterrupt):
                    raise
                response = msjson.encode(SchemaError(message=str(exc)))
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
    async def project_status() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(message="ok")

    path = socket_path or default_socket_path()
    server = await start_server(path, dispatcher)
    async with server:
        await server.serve_forever()
