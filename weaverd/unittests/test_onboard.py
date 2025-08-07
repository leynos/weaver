from __future__ import annotations

import asyncio
import typing as typ

if typ.TYPE_CHECKING:
    from pathlib import Path

import msgspec.json as msjson
import pytest

from weaver_schemas.reports import OnboardingReport
from weaverd.rpc import RPCDispatcher
from weaverd.serena_tools import create_serena_tool
from weaverd.server import start_server


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_onboard_project(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("onboard-project")
    async def onboard() -> OnboardingReport:  # pyright: ignore[reportUnusedFunction]
        tool = create_serena_tool("OnboardingTool")
        return OnboardingReport(details=tool.apply())

    sock = tmp_path / "o.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        try:
            reader, writer = await asyncio.wait_for(
                asyncio.open_unix_connection(str(sock)), timeout=5.0
            )
            writer.write(msjson.encode({"method": "onboard-project"}) + b"\n")
            await writer.drain()
            data = await asyncio.wait_for(reader.readline(), timeout=5.0)
            report = msjson.decode(data.rstrip(), type=OnboardingReport)
            text = report.details.lower()
            assert "viewing" in text and "project" in text
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
            writer.write(msjson.encode({"method": "onboard-project"}) + b"\n")
            await writer.drain()
            data = await asyncio.wait_for(reader.readline(), timeout=5.0)
            error = msjson.decode(data.rstrip())
            assert error.get("type") == "error"
        finally:
            writer.close()
            await writer.wait_closed()
    server.close()
    await server.wait_closed()
