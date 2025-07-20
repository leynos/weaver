import asyncio
import multiprocessing as mp
from io import StringIO
from pathlib import Path

import pytest
from msgspec import json

from weaver import client
from weaver_schemas.status import ProjectStatus
from weaverd.rpc import RPCDispatcher
from weaverd.server import start_server


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_rpc_call_existing_daemon(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("project-status")
    async def status() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(message="ok")

    sock = tmp_path / "d.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        buf = StringIO()
        await client.rpc_call("project-status", socket_path=sock, stdout=buf)
        assert json.decode(
            buf.getvalue().encode(), type=ProjectStatus
        ) == ProjectStatus(message="ok")
    server.close()
    await server.wait_closed()


@pytest.mark.anyio
async def test_rpc_call_autostart(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    sock = tmp_path / "auto.sock"

    def spawn(path: Path) -> mp.Process:
        def _run() -> None:
            async def _serve() -> None:
                server = await start_server(path, RPCDispatcher())
                async with server:
                    await server.serve_forever()

            asyncio.run(_serve())

        proc = mp.Process(target=_run)
        proc.start()
        return proc

    started: mp.Process | None = None

    def wrapped(path: Path) -> mp.Process:
        nonlocal started
        started = spawn(path)
        return started

    monkeypatch.setattr(client, "spawn_daemon", wrapped)

    buf = StringIO()
    await client.rpc_call("project-status", socket_path=sock, stdout=buf)
    assert started is not None
    started.terminate()
    started.join()
