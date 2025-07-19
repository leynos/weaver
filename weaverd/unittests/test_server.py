from __future__ import annotations

import asyncio
import typing as t

import pytest
from msgspec import json

from weaver_schemas.status import ProjectStatus
from weaverd.rpc import RPCDispatcher
from weaverd.server import start_server


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


if t.TYPE_CHECKING:
    from pathlib import Path


@pytest.mark.anyio
async def test_server_echoes_status(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("project-status")
    async def handler() -> ProjectStatus:
        return ProjectStatus(message="ok")

    sock = tmp_path / "d.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        reader, writer = await asyncio.open_unix_connection(str(sock))
        writer.write(json.encode({"method": "project-status"}) + b"\n")
        await writer.drain()
        data = await reader.readline()
        assert json.decode(data.rstrip(), type=ProjectStatus) == ProjectStatus(
            message="ok"
        )
        writer.close()
        await writer.wait_closed()
    server.close()
    await server.wait_closed()
