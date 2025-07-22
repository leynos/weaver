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
        try:
            reader, writer = await asyncio.wait_for(
                asyncio.open_unix_connection(str(sock)), timeout=5.0
            )
            writer.write(json.encode({"method": "onboard-project"}) + b"\n")
            await writer.drain()
            data = await asyncio.wait_for(reader.readline(), timeout=5.0)
            report = json.decode(data.rstrip(), type=OnboardingReport)
            assert report.details.startswith("You are viewing the project")
            assert len(report.details) > 20
        finally:
            writer.close()
            await writer.wait_closed()
    server.close()
    await server.wait_closed()


@pytest.mark.anyio
async def test_onboard_failure(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    class FailingTool:
        def apply(self) -> str:  # pragma: no cover - stub
            raise RuntimeError("boom")

    @dispatcher.register("onboard-project")
    async def onboard() -> OnboardingReport:  # pyright: ignore[reportUnusedFunction]
        return OnboardingReport(
            details=FailingTool().apply()
        )  # pragma: no cover - stub

    sock = tmp_path / "f.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        try:
            reader, writer = await asyncio.wait_for(
                asyncio.open_unix_connection(str(sock)), timeout=5.0
            )
            writer.write(json.encode({"method": "onboard-project"}) + b"\n")
            await writer.drain()
            data = await asyncio.wait_for(reader.readline(), timeout=5.0)
            error = json.decode(data.rstrip())
            assert error.get("type") == "error"
        finally:
            writer.close()
            await writer.wait_closed()
    server.close()
    await server.wait_closed()
