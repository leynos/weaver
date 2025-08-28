from __future__ import annotations

import asyncio
import typing as typ

import msgspec.json as msjson
import pytest

from weaver_schemas.reports import OnboardingReport
from weaverd.rpc import RPCDispatcher
from weaverd.serena_tools import (
    SerenaTool,
    UnknownSerenaToolError,
    create_serena_tool,
)
from weaverd.server import start_server

if typ.TYPE_CHECKING:
    from pathlib import Path


class _Appliable(typ.Protocol):
    """Test-local protocol for tools with ``apply``."""

    def apply(self) -> str: ...


@pytest.fixture
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_onboard_project(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("onboard-project")
    def onboard() -> OnboardingReport:  # pyright: ignore[reportUnusedFunction]
        tool = typ.cast("_Appliable", create_serena_tool(SerenaTool.ONBOARDING))
        return OnboardingReport(details=tool.apply())

    sock = tmp_path / "o.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        writer: asyncio.StreamWriter | None = None
        try:
            reader, writer = await asyncio.wait_for(
                asyncio.open_unix_connection(str(sock)), timeout=5.0
            )
            writer.write(msjson.encode({"method": "onboard-project"}) + b"\n")
            await writer.drain()
            data = await asyncio.wait_for(reader.readline(), timeout=5.0)
            report = msjson.decode(data.rstrip(), type=OnboardingReport)
            text = report.details.lower()
            assert "viewing" in text
            assert "project" in text
            assert len(report.details) > 20
        finally:
            if writer is not None:
                writer.close()
                await writer.wait_closed()
    server.close()
    await server.wait_closed()


@pytest.mark.anyio
async def test_onboard_failure(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    class FailingTool:
        def apply(self) -> str:  # pragma: no cover - stub
            class OnboardToolError(RuntimeError):
                """Test-only error to simulate onboarding failure."""

            raise OnboardToolError("boom")

    @dispatcher.register("onboard-project")
    def onboard() -> OnboardingReport:  # pyright: ignore[reportUnusedFunction]
        return OnboardingReport(
            details=FailingTool().apply()
        )  # pragma: no cover - stub

    sock = tmp_path / "f.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        writer: asyncio.StreamWriter | None = None
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
            if writer is not None:
                writer.close()
                await writer.wait_closed()
    server.close()
    await server.wait_closed()


def test_create_serena_tool_with_invalid_tool_name() -> None:
    """create_serena_tool should raise for unknown tools."""

    with pytest.raises(UnknownSerenaToolError, match="NonExistentTool"):
        create_serena_tool("NonExistentTool")
