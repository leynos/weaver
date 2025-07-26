from __future__ import annotations

import asyncio
import typing as typ

import msgspec.json as msjson
import pytest

from weaver_schemas.status import ProjectStatus
from weaverd.rpc import RPCDispatcher
from weaverd.server import start_server


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


if typ.TYPE_CHECKING:
    from pathlib import Path


@pytest.mark.anyio
async def test_server_echoes_status(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("project-status")
    async def handler() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(pid=123, rss_mb=1.0, ready=True, message="ok")

    sock = tmp_path / "d.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        reader, writer = await asyncio.open_unix_connection(str(sock))
        writer.write(msjson.encode({"method": "project-status"}) + b"\n")
        await writer.drain()
        data = await reader.readline()
        assert msjson.decode(data.rstrip(b"\n"), type=ProjectStatus) == ProjectStatus(
            pid=123, rss_mb=1.0, ready=True, message="ok"
        )
        writer.close()
        await writer.wait_closed()
    server.close()
    await server.wait_closed()


@pytest.mark.anyio
async def test_server_handles_multiple_requests(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("echo")
    async def echo(value: int) -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(
            pid=value, rss_mb=float(value), ready=True, message=str(value)
        )

    sock = tmp_path / "e.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        reader, writer = await asyncio.open_unix_connection(str(sock))
        for i in (1, 2):
            writer.write(
                msjson.encode({"method": "echo", "params": {"value": i}}) + b"\n"
            )
            await writer.drain()
            data = await reader.readline()
            assert msjson.decode(
                data.rstrip(b"\n"), type=ProjectStatus
            ) == ProjectStatus(pid=i, rss_mb=float(i), ready=True, message=str(i))
        writer.close()
        await writer.wait_closed()
    server.close()
    await server.wait_closed()
