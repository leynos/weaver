from __future__ import annotations

import typing as t

import pytest
from msgspec import json

from weaver.rpc_client import call_rpc
from weaver_schemas.status import ProjectStatus
from weaverd.rpc import RPCDispatcher
from weaverd.server import start_server

if t.TYPE_CHECKING:
    from pathlib import Path


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_call_rpc(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("project-status")
    async def handler() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(message="ok")

    sock = tmp_path / "t.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        await call_rpc(sock, "project-status")
    out = capsys.readouterr().out.strip()
    assert json.decode(out.encode(), type=ProjectStatus) == ProjectStatus(message="ok")
