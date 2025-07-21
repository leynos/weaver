from __future__ import annotations

import asyncio
import sys
import typing as t

import pytest
from msgspec import json

from weaver_schemas.reports import OnboardingReport
from weaverd.rpc import RPCDispatcher
from weaverd.server import create_onboarding_tool, start_server

sys.path.insert(0, "/root/git/serena-0.1.2/src")

if t.TYPE_CHECKING:
    from pathlib import Path


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_onboard_project(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("onboard-project")
    async def onboard() -> OnboardingReport:  # pyright: ignore[reportUnusedFunction]
        tool = create_onboarding_tool()
        return OnboardingReport(details=tool.apply())

    sock = tmp_path / "o.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        reader, writer = await asyncio.open_unix_connection(str(sock))
        writer.write(json.encode({"method": "onboard-project"}) + b"\n")
        await writer.drain()
        data = await reader.readline()
        report = json.decode(data.rstrip(), type=OnboardingReport)
        assert report.details.startswith("You are viewing the project")
        writer.close()
        await writer.wait_closed()
    server.close()
    await server.wait_closed()
